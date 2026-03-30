use std::collections::HashMap;
use synaptix_protocol::{
    registry::{get_device_profile, DeviceCapability},
    BatteryState, DeviceSettings, LightingEffect, RazerDevice, RazerProductId,
};

/// Returns the (transaction_id, led_id) pair for a device's static lighting command.
/// Derived from `razer_attr_write_matrix_effect_static_common` in razermouse_driver.c.
fn lighting_params(product_id: &RazerProductId) -> (u8, u8) {
    use crate::razer_protocol::{
        LED_BACKLIGHT, LED_ZERO, TRANSACTION_ID_COBRA, TRANSACTION_ID_DA,
        TRANSACTION_ID_KEYBOARD_WIRELESS,
    };
    match product_id {
        // Cobra Pro / Basilisk V3 Pro group: transaction_id=0x1F, ZERO_LED
        RazerProductId::CobraProWired | RazerProductId::CobraProWireless => {
            (TRANSACTION_ID_COBRA, LED_ZERO)
        }
        // BlackWidow V3 Mini HyperSpeed Wired: transaction_id=0x1F, BACKLIGHT_LED
        // Ref: razerkbd_driver.c ~line 2107
        RazerProductId::BlackWidowV3MiniHyperSpeedWired => (TRANSACTION_ID_COBRA, LED_BACKLIGHT),
        // BlackWidow V3 Mini HyperSpeed Wireless: transaction_id=0x9F, BACKLIGHT_LED
        // Ref: razerkbd_driver.c ~line 2123
        RazerProductId::BlackWidowV3MiniHyperSpeedWireless => {
            (TRANSACTION_ID_KEYBOARD_WIRELESS, LED_BACKLIGHT)
        }
        // DeathAdder V2 Pro group: transaction_id=0x3F, BACKLIGHT_LED
        RazerProductId::DeathAdderV2Pro => (TRANSACTION_ID_DA, LED_BACKLIGHT),
        // Sensible default for anything not yet explicitly mapped
        _ => (TRANSACTION_ID_DA, LED_BACKLIGHT),
    }
}

pub struct DeviceManager {
    pub(crate) devices: HashMap<String, RazerDevice>,
    lighting: HashMap<String, LightingEffect>,
    settings: HashMap<String, DeviceSettings>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            lighting: HashMap::new(),
            settings: crate::config::load_settings(),
        }
    }

    pub fn add_device(&mut self, id: String, device: RazerDevice) {
        self.devices.insert(id, device);
    }

    #[allow(dead_code)]
    pub fn get_device(&self, id: &str) -> Option<&RazerDevice> {
        self.devices.get(id)
    }

    #[allow(dead_code)]
    pub fn get_all_devices(&self) -> Vec<&RazerDevice> {
        self.devices.values().collect()
    }

    pub fn update_battery(&mut self, id: &str, state: BatteryState) {
        if let Some(device) = self.devices.get_mut(id) {
            device.battery_state = state;
        }
    }

    /// Updates the name, product ID, and connection type for a device whose
    /// physical connection has changed (e.g. dongle → cable).
    pub fn update_connection(
        &mut self,
        id: &str,
        name: String,
        product_id: synaptix_protocol::RazerProductId,
        connection_type: synaptix_protocol::ConnectionType,
    ) {
        if let Some(device) = self.devices.get_mut(id) {
            device.name = name;
            device.product_id = product_id;
            device.connection_type = connection_type;
        }
    }

    #[allow(dead_code)]
    pub fn update_lighting(&mut self, id: &str, effect: LightingEffect) {
        if self.devices.contains_key(id) {
            self.lighting.insert(id.to_string(), effect);
        }
    }

    /// Dispatches saved settings to all registered devices via USB.
    /// Called once at daemon startup after devices are registered.
    pub fn apply_saved_settings(&self) {
        for (device_id, device) in &self.devices {
            let Some(settings) = self.settings.get(device_id) else {
                continue;
            };

            let pid = device.product_id.usb_pid();
            let (txn_id, led_id) = lighting_params(&device.product_id);

            if let Some(effect) = &settings.lighting {
                let payload = match effect {
                    synaptix_protocol::LightingEffect::Static([r, g, b]) => {
                        crate::razer_protocol::build_static_color_payload(
                            txn_id, led_id, *r, *g, *b,
                        )
                    }
                    synaptix_protocol::LightingEffect::Breathing([r, g, b]) => {
                        crate::razer_protocol::build_breathing_payload(txn_id, led_id, *r, *g, *b)
                    }
                    synaptix_protocol::LightingEffect::Spectrum => {
                        crate::razer_protocol::build_spectrum_payload(txn_id, led_id)
                    }
                };
                if let Err(e) = crate::usb_backend::send_control_transfer(pid, &payload) {
                    log::warn!("[AutoApply] Lighting failed for {device_id}: {e:?}");
                } else {
                    log::info!("[AutoApply] Lighting restored for {device_id}");
                }
            }

            if let Some(dpi) = settings.dpi {
                let has_dpi = get_device_profile(pid)
                    .is_some_and(|p| p.capabilities.contains(&DeviceCapability::DpiControl));
                if has_dpi {
                    let payload = crate::razer_protocol::build_set_dpi_payload(txn_id, dpi, dpi);
                    if let Err(e) = crate::usb_backend::send_control_transfer(pid, &payload) {
                        log::warn!("[AutoApply] DPI failed for {device_id}: {e:?}");
                    } else {
                        log::info!("[AutoApply] DPI {dpi} restored for {device_id}");
                    }
                }
            }
        }
    }
}

/// D-Bus interface: exposes device state on `org.synaptix.Daemon`.
///
/// `GetDevices` returns a JSON array where each element is a serialised
/// `RazerDevice` augmented with its `device_id` key, so consumers can
/// correlate `BatteryChanged` signals back to the correct device.
#[zbus::interface(name = "org.synaptix.Daemon")]
impl DeviceManager {
    fn get_devices(&self) -> Vec<String> {
        self.devices
            .iter()
            .filter_map(|(id, device)| {
                let mut value = serde_json::to_value(device).ok()?;
                value.as_object_mut()?.insert(
                    "device_id".to_string(),
                    serde_json::Value::String(id.clone()),
                );
                Some(value.to_string())
            })
            .collect()
    }

    /// Returns the persisted `DeviceSettings` for `device_id` as a JSON string.
    ///
    /// Used by the UI on mount to hydrate its local state (DPI, lighting) from
    /// the values last written by the user. Returns `"{}"` when no settings have
    /// been saved yet for this device.
    fn get_device_state(&self, device_id: String) -> String {
        match self.settings.get(&device_id) {
            Some(s) => serde_json::to_string(s).unwrap_or_else(|_| "{}".to_string()),
            None => "{}".to_string(),
        }
    }

    /// Sets the lighting effect for a device and forwards the USB command to
    /// the physical hardware via `usb_backend`.
    ///
    /// Returns `true` if the device exists and the effect was accepted,
    /// `false` if the device ID is unknown or the JSON is malformed.
    async fn set_lighting(&mut self, device_id: String, effect_json: String) -> bool {
        println!("[SetLighting] Received command for device: {:?}", device_id);
        println!("[SetLighting] Effect JSON: {}", effect_json);

        let effect = match serde_json::from_str::<LightingEffect>(&effect_json) {
            Ok(e) => e,
            Err(err) => {
                eprintln!("[SetLighting] Failed to deserialise LightingEffect: {err:?}");
                return false;
            }
        };

        let Some(device) = self.devices.get(&device_id) else {
            eprintln!("[SetLighting] Unknown device ID: {device_id}");
            return false;
        };

        let product_id = device.product_id.usb_pid();
        let is_kraken_v4 = device.product_id == RazerProductId::KrakenV4Pro;
        let (txn_id, led_id) = lighting_params(&device.product_id);
        println!("[SetLighting] Resolved USB PID: 0x{product_id:04X}, txn_id=0x{txn_id:02X}, led_id=0x{led_id:02X}");

        self.lighting.insert(device_id.clone(), effect.clone());

        // Persist the new lighting preference.
        let entry = self.settings.entry(device_id.clone()).or_default();
        entry.lighting = Some(effect.clone());
        crate::config::save_settings(&self.settings);

        println!("[SetLighting] Dispatching {effect:?} to USB backend …");

        // Await the blocking task so errors are never swallowed.
        let result = tokio::task::spawn_blocking(move || {
            let payload = match effect {
                LightingEffect::Static([r, g, b]) => {
                    if is_kraken_v4 {
                        crate::razer_protocol::build_kraken_v4_static_payload(r, g, b)
                    } else {
                        crate::razer_protocol::build_static_color_payload(txn_id, led_id, r, g, b)
                    }
                }
                LightingEffect::Breathing([r, g, b]) => {
                    if is_kraken_v4 {
                        // Breathing protocol for Kraken V4 Pro is not yet reverse-engineered.
                        eprintln!("[SetLighting] Breathing not yet supported for Kraken V4 Pro");
                        return Err(rusb::Error::NotSupported);
                    }
                    crate::razer_protocol::build_breathing_payload(txn_id, led_id, r, g, b)
                }
                LightingEffect::Spectrum => {
                    if is_kraken_v4 {
                        eprintln!("[SetLighting] Spectrum not yet supported for Kraken V4 Pro");
                        return Err(rusb::Error::NotSupported);
                    }
                    crate::razer_protocol::build_spectrum_payload(txn_id, led_id)
                }
            };
            crate::usb_backend::send_control_transfer(product_id, &payload)
        })
        .await;

        match result {
            Ok(Ok(())) => println!("[SetLighting] USB transfer succeeded for {device_id}"),
            Ok(Err(e)) => eprintln!("[SetLighting] USB Transfer Failed: {e:?}"),
            Err(e) => eprintln!("[SetLighting] spawn_blocking panicked: {e:?}"),
        }

        true
    }

    /// Sets the DPI for a mouse device and dispatches the raw USB payload.
    ///
    /// `device_id` must match a registered device. `x` and `y` are the DPI
    /// values for each axis (valid range: 100–45 000; enforced by hardware).
    ///
    /// Returns `true` if the device exists and the command was dispatched,
    /// `false` if the device ID is unknown.
    async fn set_dpi(&mut self, device_id: String, x: u16, y: u16) -> bool {
        log::info!("[SetDpi] request — device={device_id} x={x} y={y}");

        let Some(device) = self.devices.get(&device_id) else {
            log::warn!("[SetDpi] rejected — unknown device ID: {device_id}");
            return false;
        };

        let product_id = device.product_id.usb_pid();

        // Guard: only dispatch if the registry advertises DpiControl capability.
        let has_dpi = get_device_profile(product_id)
            .is_some_and(|p| p.capabilities.contains(&DeviceCapability::DpiControl));
        if !has_dpi {
            log::warn!(
                "[SetDpi] rejected — PID=0x{product_id:04X} ({device_id}) does not advertise DpiControl capability"
            );
            return false;
        }

        let (txn_id, _) = lighting_params(&device.product_id);
        log::info!("[SetDpi] dispatching — PID=0x{product_id:04X} txn_id=0x{txn_id:02X}");

        let result = tokio::task::spawn_blocking(move || {
            let payload = crate::razer_protocol::build_set_dpi_payload(txn_id, x, y);
            crate::usb_backend::send_control_transfer(product_id, &payload)
        })
        .await;

        match result {
            Ok(Ok(())) => {
                log::info!("[SetDpi] USB transfer succeeded for {device_id}");
                // Persist the new DPI preference.
                let entry = self.settings.entry(device_id.clone()).or_default();
                entry.dpi = Some(x);
                crate::config::save_settings(&self.settings);
                true
            }
            Ok(Err(e)) => {
                log::error!("[SetDpi] USB transfer failed for {device_id}: {e:?}");
                false
            }
            Err(e) => {
                log::error!("[SetDpi] spawn_blocking panicked for {device_id}: {e:?}");
                false
            }
        }
    }

    /// Sets the sidetone volume for a headset device.
    ///
    /// `device_id` must match a registered device. `level` is clamped to 0–100.
    /// Returns `false` if the device is unknown or doesn't advertise sidetone support.
    ///
    /// ⚠️  USB payload based on Kraken V3 baseline — Wireshark verification needed
    ///     for Kraken V4 Pro (PID 0x0568).
    async fn set_sidetone(&mut self, device_id: String, level: u8) -> bool {
        log::info!("[SetSidetone] request — device={device_id} level={level}");

        let Some(device) = self.devices.get(&device_id) else {
            log::warn!("[SetSidetone] rejected — unknown device ID: {device_id}");
            return false;
        };
        let pid = device.product_id.usb_pid();

        // Guard: only dispatch if the registry advertises Sidetone capability.
        let has_sidetone = get_device_profile(pid)
            .is_some_and(|p| p.capabilities.contains(&DeviceCapability::Sidetone));
        if !has_sidetone {
            log::warn!(
                "[SetSidetone] rejected — PID={pid:#06x} ({device_id}) does not advertise Sidetone capability"
            );
            return false;
        }

        let clamped = level.min(100);
        log::info!("[SetSidetone] dispatching — PID={pid:#06x} level={clamped}");

        let result = tokio::task::spawn_blocking(move || {
            let payload = crate::razer_protocol::build_set_sidetone_payload(clamped);
            crate::usb_backend::send_control_transfer(pid, &payload)
        })
        .await;

        match result {
            Ok(Ok(())) => {
                log::info!("[SetSidetone] USB transfer succeeded for {device_id}");
                true
            }
            Ok(Err(e)) => {
                log::error!("[SetSidetone] USB transfer failed for {device_id}: {e:?}");
                false
            }
            Err(e) => {
                log::error!("[SetSidetone] spawn_blocking panicked for {device_id}: {e:?}");
                false
            }
        }
    }

    /// Sets the haptic feedback intensity for a HyperSense-equipped headset.
    ///
    /// `device_id` must match a registered device. `level` 0 disables haptics;
    /// 1–100 sets intensity. Returns `false` if the device is unknown or doesn't
    /// advertise haptic feedback support.
    ///
    /// ⚠️  USB payload based on Kraken V3 HyperSense baseline — Wireshark verification
    ///     needed for Kraken V4 Pro (PID 0x0568).
    async fn set_haptic_intensity(&mut self, device_id: String, level: u8) -> bool {
        log::info!("[SetHapticIntensity] request — device={device_id} level={level}");

        let Some(device) = self.devices.get(&device_id) else {
            log::warn!("[SetHapticIntensity] rejected — unknown device ID: {device_id}");
            return false;
        };
        let pid = device.product_id.usb_pid();

        // Guard: only dispatch if the registry advertises HapticFeedback capability.
        let has_haptics = get_device_profile(pid)
            .is_some_and(|p| p.capabilities.contains(&DeviceCapability::HapticFeedback));
        if !has_haptics {
            log::warn!(
                "[SetHapticIntensity] rejected — PID={pid:#06x} ({device_id}) does not advertise HapticFeedback capability"
            );
            return false;
        }

        let clamped = level.min(100);
        log::info!("[SetHapticIntensity] dispatching — PID={pid:#06x} level={clamped}");

        let result = tokio::task::spawn_blocking(move || {
            if pid == 0x0568 {
                // Kraken V4 Pro OLED Hub: 64-byte proprietary HID report on
                // Interface 4, wValue=0x0202. Wireshark-verified protocol path.
                let payload = crate::razer_protocol::build_haptic_report(clamped);
                crate::usb_backend::send_haptic_report(pid, &payload)
            } else {
                // Legacy 90-byte Razer protocol for Kraken V3 HyperSense and
                // other HapticFeedback-capable headsets.
                let payload = crate::razer_protocol::build_set_haptic_payload(clamped);
                crate::usb_backend::send_control_transfer(pid, &payload)
            }
        })
        .await;

        match result {
            Ok(Ok(())) => {
                log::info!("[SetHapticIntensity] USB transfer succeeded for {device_id}");
                true
            }
            Ok(Err(e)) => {
                log::error!("[SetHapticIntensity] USB transfer failed for {device_id}: {e:?}");
                false
            }
            Err(e) => {
                log::error!("[SetHapticIntensity] spawn_blocking panicked for {device_id}: {e:?}");
                false
            }
        }
    }

    /// Emitted whenever a device's battery state changes.
    /// `new_state_json` is the serde-JSON serialisation of `BatteryState`.
    #[zbus(signal)]
    pub async fn battery_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        device_id: &str,
        new_state_json: &str,
    ) -> zbus::Result<()>;

    /// Emitted whenever a device's physical connection type changes
    /// (e.g. USB cable plugged in while the dongle was active).
    /// `connection_type_json` is the serde-JSON serialisation of `ConnectionType`.
    #[zbus(signal)]
    pub async fn connection_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        device_id: &str,
        connection_type_json: &str,
    ) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only D-Bus proxy: only needs the signal, not the full interface.
    #[zbus::proxy(
        interface = "org.synaptix.Daemon",
        default_service = "org.synaptix.DaemonTest",
        default_path = "/org/synaptix/Daemon"
    )]
    trait TestDaemon {
        #[zbus(signal)]
        fn battery_changed(&self, device_id: &str, new_state_json: &str) -> zbus::Result<()>;
    }

    fn mock_device() -> RazerDevice {
        RazerDevice {
            name: "Razer DeathAdder V2 Pro".to_string(),
            product_id: RazerProductId::DeathAdderV2Pro,
            battery_state: BatteryState::Discharging(75),
            capabilities: vec![],
            connection_type: synaptix_protocol::ConnectionType::Wired,
        }
    }

    #[test]
    fn test_add_and_retrieve_device() {
        let mut manager = DeviceManager::new();
        let device = mock_device();

        manager.add_device("da-v2-pro".to_string(), device.clone());

        let retrieved = manager
            .get_device("da-v2-pro")
            .expect("device should exist");
        assert_eq!(retrieved.name, device.name);
        assert_eq!(retrieved.product_id, device.product_id);
        assert_eq!(retrieved.battery_state, device.battery_state);
    }

    #[test]
    fn test_update_battery_state() {
        let mut manager = DeviceManager::new();
        manager.add_device("da-v2-pro".to_string(), mock_device());

        manager.update_battery("da-v2-pro", BatteryState::Charging(80));

        let device = manager
            .get_device("da-v2-pro")
            .expect("device should exist");
        assert_eq!(device.battery_state, BatteryState::Charging(80));
    }

    /// Integration test: verifies that `BatteryChanged` is emitted over D-Bus
    /// and received by a subscribing client with the correct arguments.
    ///
    /// Uses a dedicated service name (`org.synaptix.DaemonTest`) to avoid
    /// interfering with a running production daemon.
    #[tokio::test]
    async fn test_battery_signal_emission() {
        use futures_util::StreamExt;

        // ── Server side ────────────────────────────────────────────────────
        let mut manager = DeviceManager::new();
        manager.add_device("da-v2-pro".to_string(), mock_device());

        let server_conn = zbus::connection::Builder::session()
            .unwrap()
            .name("org.synaptix.DaemonTest")
            .unwrap()
            .serve_at("/org/synaptix/Daemon", manager)
            .unwrap()
            .build()
            .await
            .unwrap();

        // ── Client side ────────────────────────────────────────────────────
        let client_conn = zbus::Connection::session().await.unwrap();
        let proxy = TestDaemonProxy::new(&client_conn).await.unwrap();
        let mut signal_stream = proxy.receive_battery_changed().await.unwrap();

        // ── Action ─────────────────────────────────────────────────────────
        let new_state = BatteryState::Charging(85);
        let state_json = serde_json::to_string(&new_state).unwrap();

        let iface_ref = server_conn
            .object_server()
            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
            .await
            .unwrap();

        {
            let mut iface = iface_ref.get_mut().await;
            iface.update_battery("da-v2-pro", new_state);
        }

        DeviceManager::battery_changed(iface_ref.signal_emitter(), "da-v2-pro", &state_json)
            .await
            .unwrap();

        // ── Assertion ──────────────────────────────────────────────────────
        let signal = tokio::time::timeout(std::time::Duration::from_secs(5), signal_stream.next())
            .await
            .expect("timed out waiting for BatteryChanged signal")
            .expect("signal stream ended unexpectedly");

        let args = signal.args().expect("failed to parse signal args");
        assert_eq!(*args.device_id(), "da-v2-pro");

        let received_state: BatteryState =
            serde_json::from_str(args.new_state_json()).expect("failed to parse BatteryState");
        assert_eq!(received_state, BatteryState::Charging(85));
    }

    // ── lighting_params routing tests ─────────────────────────────────────────

    /// Wired keyboard must use TRANSACTION_ID_COBRA (0x1F) and LED_BACKLIGHT (0x05).
    /// Derived from razerkbd_driver.c lines ~2107:
    ///   request.transaction_id.id = 0x1F  (wired path)
    ///   razer_chroma_extended_matrix_effect_static(VARSTORE, BACKLIGHT_LED, ...)
    #[test]
    fn test_lighting_params_blackwidow_wired_uses_cobra_txn_backlight_led() {
        use crate::razer_protocol::{LED_BACKLIGHT, TRANSACTION_ID_COBRA};
        let (txn_id, led_id) = lighting_params(&RazerProductId::BlackWidowV3MiniHyperSpeedWired);
        assert_eq!(
            txn_id, TRANSACTION_ID_COBRA,
            "Wired keyboard must use TRANSACTION_ID_COBRA (0x1F)"
        );
        assert_eq!(
            led_id, LED_BACKLIGHT,
            "Wired keyboard must target LED_BACKLIGHT (0x05)"
        );
    }

    /// Wireless keyboard must use TRANSACTION_ID_KEYBOARD_WIRELESS (0x9F) and LED_BACKLIGHT (0x05).
    /// Derived from razerkbd_driver.c lines ~2123:
    ///   request.transaction_id.id = 0x9F  (wireless path)
    ///   razer_chroma_extended_matrix_effect_static(VARSTORE, BACKLIGHT_LED, ...)
    #[test]
    fn test_lighting_params_blackwidow_wireless_uses_wireless_txn_backlight_led() {
        use crate::razer_protocol::{LED_BACKLIGHT, TRANSACTION_ID_KEYBOARD_WIRELESS};
        let (txn_id, led_id) = lighting_params(&RazerProductId::BlackWidowV3MiniHyperSpeedWireless);
        assert_eq!(
            txn_id, TRANSACTION_ID_KEYBOARD_WIRELESS,
            "Wireless keyboard must use TRANSACTION_ID_KEYBOARD_WIRELESS (0x9F)"
        );
        assert_eq!(
            led_id, LED_BACKLIGHT,
            "Wireless keyboard must target LED_BACKLIGHT (0x05)"
        );
    }
}
