use std::collections::HashMap;
#[cfg(not(test))]
use synaptix_protocol::{BatteryState, RazerDevice};
#[cfg(test)]
use synaptix_protocol::{BatteryState, RazerDevice, RazerProductId};

pub struct DeviceManager {
    devices: HashMap<String, RazerDevice>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, id: String, device: RazerDevice) {
        self.devices.insert(id, device);
    }

    pub fn get_device(&self, id: &str) -> Option<&RazerDevice> {
        self.devices.get(id)
    }

    pub fn get_all_devices(&self) -> Vec<&RazerDevice> {
        self.devices.values().collect()
    }

    pub fn update_battery(&mut self, id: &str, state: BatteryState) {
        if let Some(device) = self.devices.get_mut(id) {
            device.battery_state = state;
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

    /// Emitted whenever a device's battery state changes.
    /// `new_state_json` is the serde-JSON serialisation of `BatteryState`.
    #[zbus(signal)]
    pub async fn battery_changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        device_id: &str,
        new_state_json: &str,
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
        }
    }

    #[test]
    fn test_add_and_retrieve_device() {
        let mut manager = DeviceManager::new();
        let device = mock_device();

        manager.add_device("da-v2-pro".to_string(), device.clone());

        let retrieved = manager.get_device("da-v2-pro").expect("device should exist");
        assert_eq!(retrieved.name, device.name);
        assert_eq!(retrieved.product_id, device.product_id);
        assert_eq!(retrieved.battery_state, device.battery_state);
    }

    #[test]
    fn test_update_battery_state() {
        let mut manager = DeviceManager::new();
        manager.add_device("da-v2-pro".to_string(), mock_device());

        manager.update_battery("da-v2-pro", BatteryState::Charging(80));

        let device = manager.get_device("da-v2-pro").expect("device should exist");
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

        DeviceManager::battery_changed(&iface_ref.signal_emitter(), "da-v2-pro", &state_json)
            .await
            .unwrap();

        // ── Assertion ──────────────────────────────────────────────────────
        let signal = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            signal_stream.next(),
        )
        .await
        .expect("timed out waiting for BatteryChanged signal")
        .expect("signal stream ended unexpectedly");

        let args = signal.args().expect("failed to parse signal args");
        assert_eq!(*args.device_id(), "da-v2-pro");

        let received_state: BatteryState =
            serde_json::from_str(args.new_state_json()).expect("failed to parse BatteryState");
        assert_eq!(received_state, BatteryState::Charging(85));
    }
}
