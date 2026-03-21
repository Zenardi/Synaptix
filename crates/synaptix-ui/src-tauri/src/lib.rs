use synaptix_protocol::RazerDevice;

/// Proxy for the `org.synaptix.Daemon` D-Bus interface exposed by
/// `synaptix-daemon`. The Tauri layer is strictly a consumer — it never
/// touches hardware directly.
#[zbus::proxy(
    interface = "org.synaptix.Daemon",
    default_service = "org.synaptix.Daemon",
    default_path = "/org/synaptix/Daemon"
)]
trait SynaptixDaemon {
    fn get_devices(&self) -> zbus::Result<Vec<String>>;
}

/// Tauri IPC command: connects to the session bus, queries the daemon, and
/// returns deserialised `RazerDevice` values to the React frontend.
#[tauri::command]
async fn get_razer_devices() -> Result<Vec<RazerDevice>, String> {
    let conn = zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;

    let proxy = SynaptixDaemonProxy::new(&conn)
        .await
        .map_err(|e| e.to_string())?;

    let device_jsons = proxy
        .get_devices()
        .await
        .map_err(|e| e.to_string())?;

    let devices = device_jsons
        .iter()
        .filter_map(|json| serde_json::from_str::<RazerDevice>(json).ok())
        .collect();

    Ok(devices)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_razer_devices])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
