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

    fn set_lighting(&self, device_id: &str, effect_json: &str) -> zbus::Result<bool>;

    fn set_dpi(&self, device_id: &str, x: u16, y: u16) -> zbus::Result<bool>;

    /// Signal emitted by the daemon whenever a device's battery state changes.
    #[zbus(signal)]
    fn battery_changed(&self, device_id: &str, new_state_json: &str) -> zbus::Result<()>;
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
}

/// Payload carried by the Tauri `device-battery-updated` event.
#[derive(Clone, serde::Serialize)]
struct BatteryUpdatePayload {
    device_id: String,
    battery_state: BatteryState,
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

/// Background task: subscribes to the daemon's `BatteryChanged` D-Bus signal
/// and forwards each event to the React frontend via Tauri's event system.
///
/// On startup the daemon may not be registered yet. We call
/// `ensure_daemon_running` before subscribing and, if the signal subscription
/// itself still fails with `ServiceUnknown`, attempt one more auto-start
/// before propagating the error.
async fn listen_for_signals(
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = zbus::Connection::session().await?;

    // Best-effort: wake the daemon before we try to subscribe.
    ensure_daemon_running(&conn).await;

    let proxy = SynaptixDaemonProxy::new(&conn).await?;

    let mut stream = match proxy.receive_battery_changed().await {
        Ok(s) => s,
        Err(e) => {
            // A ServiceUnknown means the daemon was still not up after the
            // first ensure call (e.g., slow machine). Try one more time.
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
                let payload = BatteryUpdatePayload {
                    device_id,
                    battery_state,
                };
                // emit() broadcasts to all windows — Tauri v2 API.
                app.emit("device-battery-updated", payload).ok();
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
            // Spawn the D-Bus signal listener for the lifetime of the app.
            tauri::async_runtime::spawn(async move {
                if let Err(e) = listen_for_signals(handle).await {
                    eprintln!("D-Bus signal listener failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_razer_devices,
            set_device_lighting,
            set_device_dpi,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
