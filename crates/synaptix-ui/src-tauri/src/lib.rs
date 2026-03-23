use futures_util::StreamExt;
use synaptix_protocol::{BatteryState, LightingEffect};
use tauri::{AppHandle, Emitter, Manager};

/// Proxy for the `org.synaptix.Daemon` D-Bus interface.
/// Strictly a consumer — no hardware logic lives here.
#[zbus::proxy(
    interface = "org.synaptix.Daemon",
    default_service = "org.synaptix.Daemon",
    default_path = "/org/synaptix/Daemon"
)]
trait SynaptixDaemon {
    fn get_devices(&self) -> zbus::Result<Vec<String>>;

    fn get_device_state(&self, device_id: &str) -> zbus::Result<String>;

    fn set_lighting(&self, device_id: &str, effect_json: &str) -> zbus::Result<bool>;

    fn set_dpi(&self, device_id: &str, x: u16, y: u16) -> zbus::Result<bool>;

    fn set_haptic_intensity(&self, device_id: &str, level: u8) -> zbus::Result<bool>;

    fn set_sidetone(&self, device_id: &str, level: u8) -> zbus::Result<bool>;

    /// Signal emitted by the daemon whenever a device's battery state changes.
    #[zbus(signal)]
    fn battery_changed(&self, device_id: &str, new_state_json: &str) -> zbus::Result<()>;

    /// Signal emitted whenever a device's physical connection type changes.
    #[zbus(signal)]
    fn connection_changed(&self, device_id: &str, connection_type_json: &str) -> zbus::Result<()>;
}

/// Flat representation returned to the React frontend. The daemon injects
/// `device_id` into the JSON before sending, so we deserialise it here.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceEntry {
    pub device_id: String,
    pub name: String,
    pub product_id: serde_json::Value,
    pub battery_state: BatteryState,
    pub capabilities: Vec<serde_json::Value>,
    #[serde(default)]
    pub connection_type: synaptix_protocol::ConnectionType,
}

/// Payload carried by the Tauri `device-battery-updated` event.
#[derive(Clone, serde::Serialize)]
struct BatteryUpdatePayload {
    device_id: String,
    battery_state: BatteryState,
}

/// Payload carried by the Tauri `device-connection-changed` event.
#[derive(Clone, serde::Serialize)]
struct ConnectionUpdatePayload {
    device_id: String,
    connection_type: synaptix_protocol::ConnectionType,
}

/// Returns `true` if `org.synaptix.Daemon` has a registered name on the
/// session bus right now.
async fn daemon_is_on_bus(conn: &zbus::Connection) -> bool {
    conn.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "GetNameOwner",
        &("org.synaptix.Daemon",),
    )
    .await
    .is_ok()
}

/// If the daemon is absent, attempts to start the systemd user service and
/// waits 500 ms for it to bind to D-Bus. Idempotent — safe to call multiple
/// times or when the daemon is already running.
async fn ensure_daemon_running(conn: &zbus::Connection) {
    if daemon_is_on_bus(conn).await {
        return;
    }

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "start", "synaptix-daemon.service"])
        .status();

    // Give the daemon time to register its well-known name on the bus.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// Tauri IPC command: fetches the current device list from the daemon.
#[tauri::command]
async fn get_razer_devices() -> Result<Vec<DeviceEntry>, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;

    // Auto-start the daemon if it is not yet running (e.g., after a fresh
    // .deb install where the user service has been enabled but not started).
    ensure_daemon_running(&conn).await;

    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;

    let jsons = proxy.get_devices().await.map_err(|e| e.to_string())?;

    let entries = jsons
        .iter()
        .filter_map(|json| serde_json::from_str::<DeviceEntry>(json).ok())
        .collect();

    Ok(entries)
}

/// Tauri IPC command: fetches persisted settings (DPI, lighting) for a device.
///
/// Returns a JSON string matching `DeviceSettings` — `{}` if nothing saved yet.
/// The React frontend calls this on mount to hydrate its local state.
#[tauri::command]
async fn get_device_state(device_id: String) -> Result<String, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    proxy
        .get_device_state(&device_id)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri IPC command: applies a lighting effect to a device via the daemon.
#[tauri::command]
async fn set_device_lighting(device_id: String, effect: LightingEffect) -> Result<bool, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;

    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;

    let effect_json = serde_json::to_string(&effect).map_err(|e| e.to_string())?;
    proxy
        .set_lighting(&device_id, &effect_json)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri IPC command: sets the DPI for a device via the daemon (x == y for uniform DPI).
#[tauri::command]
async fn set_device_dpi(device_id: String, dpi: u16) -> Result<bool, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    proxy
        .set_dpi(&device_id, dpi, dpi)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri IPC command: sets haptic feedback intensity (0 = off, 1–100 = intensity).
#[tauri::command]
async fn set_haptic_intensity(device_id: String, level: u8) -> Result<bool, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    proxy
        .set_haptic_intensity(&device_id, level)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri IPC command: enables or disables haptic feedback.
/// Disabling sends intensity 0; enabling is a no-op (intensity slider drives the level).
#[tauri::command]
async fn set_haptics_enabled(device_id: String, enabled: bool) -> Result<bool, String> {
    if !enabled {
        return set_haptic_intensity(device_id, 0).await;
    }
    Ok(true)
}

/// Tauri IPC command: sets the sidetone volume (0–100) via the daemon.
#[tauri::command]
async fn set_sidetone(device_id: String, level: u8) -> Result<bool, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;
    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;
    proxy
        .set_sidetone(&device_id, level)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri IPC command: stub for THX Spatial Audio toggle.
/// ⚠️  No USB protocol yet — requires Wireshark capture.
#[tauri::command]
async fn set_thx_spatial(_device_id: String, _enabled: bool) -> Result<bool, String> {
    eprintln!("[set_thx_spatial] stub — no USB protocol implemented yet");
    Ok(false)
}

/// Tauri IPC command: stub for mic mute toggle.
/// ⚠️  No USB protocol yet — requires Wireshark capture.
#[tauri::command]
async fn set_mic_mute(_device_id: String, _muted: bool) -> Result<bool, String> {
    eprintln!("[set_mic_mute] stub — no USB protocol implemented yet");
    Ok(false)
}

/// Tauri IPC command: sets headset output volume (0–100) via PipeWire (`wpctl`).
///
/// Volume on a USB Audio Class device is managed by the OS audio stack, not by
/// proprietary USB commands. We discover the Razer audio sink at runtime by
/// parsing `wpctl status` so the node ID (which changes across reboots) is never
/// hard-coded.
///
/// Uses the absolute path `/usr/bin/wpctl` because Tauri's subprocess environment
/// may not include `/usr/bin` in PATH.
#[tauri::command]
async fn set_volume(_device_id: String, level: u8) -> Result<bool, String> {
    let level = level.min(100);
    let node_id = find_razer_stereo_node_id().await?;
    let vol_arg = format!("{level}%");
    let result = tokio::process::Command::new("/usr/bin/wpctl")
        .args(["set-volume", &node_id.to_string(), &vol_arg])
        .status()
        .await
        .map_err(|e| format!("[set_volume] wpctl set-volume failed: {e}"))?;

    if !result.success() {
        return Err(format!(
            "[set_volume] wpctl exited with status {}",
            result.code().unwrap_or(-1)
        ));
    }

    eprintln!("[set_volume] node={node_id} → {level}%");
    Ok(true)
}

/// Tauri IPC command: reads the current headset output volume (0–100) from PipeWire.
///
/// Used by the frontend to initialize the volume slider from the real system state
/// so the slider position always matches what the OS is actually outputting.
#[tauri::command]
async fn get_volume(_device_id: String) -> Result<u8, String> {
    let node_id = find_razer_stereo_node_id().await?;
    let output = tokio::process::Command::new("/usr/bin/wpctl")
        .args(["get-volume", &node_id.to_string()])
        .output()
        .await
        .map_err(|e| format!("[get_volume] wpctl get-volume failed: {e}"))?;

    // Output format: "Volume: 0.47\n"  (may also contain "[MUTED]")
    let stdout = String::from_utf8_lossy(&output.stdout);
    let vol_f: f32 = stdout
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("[get_volume] could not parse wpctl output: {stdout:?}"))?;

    // Clamp to 0–100 and round to nearest integer.
    Ok((vol_f * 100.0).round().clamp(0.0, 100.0) as u8)
}

/// Discovers the PipeWire node ID for the Razer Kraken stereo output sink by
/// parsing `wpctl status`. The node ID is not stable across reboots, so it must
/// be looked up each time.
async fn find_razer_stereo_node_id() -> Result<u32, String> {
    let status = tokio::process::Command::new("/usr/bin/wpctl")
        .arg("status")
        .output()
        .await
        .map_err(|e| format!("[wpctl] failed to run wpctl status: {e}"))?;

    let stdout = String::from_utf8_lossy(&status.stdout);

    // Lines inside the Sinks section look like:
    //   "│  *   69. Razer Kraken V4 Pro Stereo          [vol: 0.80]"
    // We pick the stereo sink (not Mono) for the headphone output.
    stdout
        .lines()
        .find(|l| {
            let low = l.to_lowercase();
            (low.contains("razer") || low.contains("kraken"))
                && low.contains("stereo")
                && !low.contains("source")
        })
        .and_then(|line| {
            line.split_whitespace()
                .find_map(|tok| tok.trim_end_matches('.').parse::<u32>().ok())
        })
        .ok_or_else(|| {
            "Razer Kraken stereo sink not found in wpctl status — is the headset connected?"
                .to_string()
        })
}

/// Background task: subscribes to the daemon's `BatteryChanged` D-Bus signal
/// and forwards each event to the React frontend via Tauri's event system.
async fn listen_for_battery_signals(
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = zbus::Connection::session().await?;
    ensure_daemon_running(&conn).await;
    let proxy = SynaptixDaemonProxy::new(&conn).await?;

    let mut stream = match proxy.receive_battery_changed().await {
        Ok(s) => s,
        Err(e) => {
            let is_absent = matches!(
                &e,
                zbus::Error::MethodError(name, ..)
                    if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown"
            );
            if is_absent {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", "synaptix-daemon.service"])
                    .status();
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                proxy.receive_battery_changed().await?
            } else {
                return Err(Box::new(e));
            }
        }
    };

    while let Some(signal) = stream.next().await {
        if let Ok(args) = signal.args() {
            let device_id = args.device_id().to_string();
            let new_state_json = args.new_state_json().to_string();
            if let Ok(battery_state) = serde_json::from_str::<BatteryState>(&new_state_json) {
                app.emit(
                    "device-battery-updated",
                    BatteryUpdatePayload {
                        device_id,
                        battery_state,
                    },
                )
                .ok();
            }
        }
    }
    Ok(())
}

/// Background task: subscribes to the daemon's `ConnectionChanged` D-Bus signal
/// and forwards each event to the React frontend via Tauri's event system.
async fn listen_for_connection_signals(
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = zbus::Connection::session().await?;
    ensure_daemon_running(&conn).await;
    let proxy = SynaptixDaemonProxy::new(&conn).await?;

    let mut stream = match proxy.receive_connection_changed().await {
        Ok(s) => s,
        Err(e) => {
            let is_absent = matches!(
                &e,
                zbus::Error::MethodError(name, ..)
                    if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown"
            );
            if is_absent {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "start", "synaptix-daemon.service"])
                    .status();
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                proxy.receive_connection_changed().await?
            } else {
                return Err(Box::new(e));
            }
        }
    };

    while let Some(signal) = stream.next().await {
        if let Ok(args) = signal.args() {
            let device_id = args.device_id().to_string();
            let ct_json = args.connection_type_json().to_string();
            if let Ok(connection_type) =
                serde_json::from_str::<synaptix_protocol::ConnectionType>(&ct_json)
            {
                app.emit(
                    "device-connection-changed",
                    ConnectionUpdatePayload {
                        device_id,
                        connection_type,
                    },
                )
                .ok();
            }
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Set the window icon at runtime so the GNOME/X11 taskbar shows the
            // Synaptix icon instead of the default cog. bundle.icon only affects
            // the packaged .deb — this is needed for the running process.
            #[cfg(target_os = "linux")]
            if let (Some(window), Some(icon)) = (
                app.get_webview_window("main"),
                app.default_window_icon().cloned(),
            ) {
                window.set_icon(icon).expect("failed to set window icon");
            }

            let handle = app.handle().clone();
            let handle2 = app.handle().clone();
            // Spawn both D-Bus signal listeners independently for the app lifetime.
            tauri::async_runtime::spawn(async move {
                if let Err(e) = listen_for_battery_signals(handle).await {
                    eprintln!("Battery signal listener failed: {e}");
                }
            });
            tauri::async_runtime::spawn(async move {
                if let Err(e) = listen_for_connection_signals(handle2).await {
                    eprintln!("Connection signal listener failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_razer_devices,
            get_device_state,
            set_device_lighting,
            set_device_dpi,
            set_haptic_intensity,
            set_haptics_enabled,
            set_sidetone,
            set_thx_spatial,
            set_mic_mute,
            set_volume,
            get_volume,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
