use futures_util::StreamExt;
use synaptix_protocol::{BatteryState, LightingEffect};
use tauri::{AppHandle, Emitter};

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
}

/// Payload carried by the Tauri `device-battery-updated` event.
#[derive(Clone, serde::Serialize)]
struct BatteryUpdatePayload {
    device_id: String,
    battery_state: BatteryState,
}

/// Tauri IPC command: fetches the current device list from the daemon.
#[tauri::command]
async fn get_razer_devices() -> Result<Vec<DeviceEntry>, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;

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

/// Background task: subscribes to the daemon's `BatteryChanged` D-Bus signal
/// and forwards each event to the React frontend via Tauri's event system.
async fn listen_for_signals(
    app: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = zbus::Connection::session().await?;
    let proxy = SynaptixDaemonProxy::new(&conn).await?;
    let mut stream = proxy.receive_battery_changed().await?;

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
            let handle = app.handle().clone();
            // Spawn the D-Bus signal listener for the lifetime of the app.
            tauri::async_runtime::spawn(async move {
                if let Err(e) = listen_for_signals(handle).await {
                    eprintln!("D-Bus signal listener failed: {e}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_razer_devices, set_device_lighting])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
