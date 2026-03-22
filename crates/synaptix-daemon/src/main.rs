mod config;
mod device_manager;
mod razer_protocol;
mod tray;
mod usb_backend;

use device_manager::DeviceManager;
use synaptix_protocol::{
    registry::get_device_profile,
    BatteryState, ConnectionType, RazerDevice, RazerProductId,
};

// PIDs for the Cobra Pro, ordered wired-first so we prefer the cable
// connection when both happen to be enumerated simultaneously.
const COBRA_PRO_PIDS: &[(u16, ConnectionType)] = &[
    (0x00AF, ConnectionType::Wired),   // USB cable
    (0x00B0, ConnectionType::Dongle),  // HyperSpeed dongle
];

/// Probe USB for the first Cobra Pro PID that is currently attached and
/// return `(pid, product_id_enum, connection_type, name)`.
fn detect_cobra_pro() -> (u16, RazerProductId, ConnectionType, String) {
    let candidate_pids: Vec<u16> = COBRA_PRO_PIDS.iter().map(|(p, _)| *p).collect();
    let found_pid = usb_backend::detect_connected_pid(&candidate_pids);

    let (pid, conn_type) = found_pid
        .and_then(|p| {
            COBRA_PRO_PIDS
                .iter()
                .find(|(cp, _)| *cp == p)
                .map(|(cp, ct)| (*cp, ct.clone()))
        })
        .unwrap_or_else(|| {
            // Nothing on USB — assume Bluetooth (not enumerable via rusb).
            (0x00B0, ConnectionType::Bluetooth)
        });

    let product_id = match pid {
        0x00AF => RazerProductId::CobraProWired,
        _ => RazerProductId::CobraProWireless,
    };
    let name = get_device_profile(pid)
        .map(|p| p.name)
        .unwrap_or_else(|| "Razer Cobra Pro".to_string());

    (pid, product_id, conn_type, name)
}
use tray::TrayUpdate;

/// Converts a `BatteryState` into a `(percentage, is_charging)` pair for the tray.
fn battery_to_pct(state: &BatteryState) -> (u8, bool) {
    match state {
        BatteryState::Charging(pct) => (*pct, true),
        BatteryState::Discharging(pct) => (*pct, false),
        BatteryState::Full => (100, true),
    }
}

/// Runs the tokio async daemon: USB polling loop + D-Bus server.
///
/// Sends `TrayUpdate` messages over `tx` whenever the battery state changes so
/// the GTK tray icon on the main thread can update without blocking.
async fn run_daemon(tx: std::sync::mpsc::Sender<TrayUpdate>) {
    let txn_id = razer_protocol::TRANSACTION_ID_COBRA;
    let wait_us = razer_protocol::WAIT_NEW_RECEIVER_US;

    // Detect which Cobra Pro PID is on the bus right now.
    let (mut cobra_pid, mut cobra_product_id, mut cobra_conn, mut cobra_name) =
        tokio::task::spawn_blocking(detect_cobra_pro)
            .await
            .unwrap_or_else(|_| (0x00B0, RazerProductId::CobraProWireless, ConnectionType::Bluetooth, "Razer Cobra Pro".to_string()));

    log::info!("[Detect] Cobra Pro on USB: PID=0x{cobra_pid:04X} connection={}", cobra_conn.label());

    // Query real battery state on startup so the tray shows real data immediately.
    let initial_battery = {
        let pid = cobra_pid;
        tokio::task::spawn_blocking(move || usb_backend::query_battery(pid, txn_id, wait_us))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                log::warn!("[Battery] Startup query failed — defaulting to 0 %.");
                BatteryState::Discharging(0)
            })
    };

    log::info!("[Battery] Startup state: {initial_battery:?}");

    // Push the initial reading to the tray immediately.
    let (pct, charging) = battery_to_pct(&initial_battery);
    tx.send(TrayUpdate {
        device_name: cobra_name.clone(),
        percentage: pct,
        is_charging: charging,
    })
    .ok();

    let mut manager = DeviceManager::new();
    let cobra_capabilities = get_device_profile(cobra_pid)
        .map(|p| p.capabilities)
        .unwrap_or_default();
    manager.add_device(
        "cobra-pro".to_string(),
        RazerDevice {
            name: cobra_name.clone(),
            product_id: cobra_product_id.clone(),
            battery_state: initial_battery.clone(),
            capabilities: cobra_capabilities,
            connection_type: cobra_conn.clone(),
        },
    );

    // Auto-apply any persisted settings (lighting, DPI) to hardware at startup.
    tokio::task::block_in_place(|| manager.apply_saved_settings());

    let conn = match zbus::connection::Builder::session()
        .and_then(|b| b.name("org.synaptix.Daemon"))
        .and_then(|b| b.serve_at("/org/synaptix/Daemon", manager))
    {
        Ok(builder) => match builder.build().await {
            Ok(c) => c,
            Err(e) => {
                log::error!("[Daemon] D-Bus connection failed: {e:?}");
                return;
            }
        },
        Err(e) => {
            log::error!("[Daemon] D-Bus builder failed: {e:?}");
            return;
        }
    };

    log::info!("Synaptix Daemon running on org.synaptix.Daemon at /org/synaptix/Daemon");

    let mut last_emitted: Option<BatteryState> = Some(initial_battery);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        // Re-detect the connection type — user may have switched from dongle to cable.
        let (new_pid, new_product_id, new_conn, new_name) =
            tokio::task::spawn_blocking(detect_cobra_pro)
                .await
                .unwrap_or_else(|_| (cobra_pid, cobra_product_id.clone(), cobra_conn.clone(), cobra_name.clone()));

        if new_pid != cobra_pid || new_conn != cobra_conn {
            log::info!(
                "[Detect] Connection changed: {} → {} (PID 0x{:04X} → 0x{:04X})",
                cobra_conn.label(), new_conn.label(), cobra_pid, new_pid
            );
            cobra_pid = new_pid;
            cobra_product_id = new_product_id;
            cobra_conn = new_conn.clone();
            cobra_name = new_name;

            let iface_ref = conn
                .object_server()
                .interface::<_, DeviceManager>("/org/synaptix/Daemon")
                .await
                .expect("DeviceManager interface must be registered");
            {
                let mut iface = iface_ref.get_mut().await;
                iface.update_connection(
                    "cobra-pro",
                    cobra_name.clone(),
                    cobra_product_id.clone(),
                    new_conn,
                );
            }
        }

        let pid = cobra_pid;
        let new_state = match tokio::task::spawn_blocking(move || {
            usb_backend::query_battery(pid, txn_id, wait_us)
        })
        .await
        {
            Ok(Ok(state)) => state,
            Ok(Err(e)) => {
                log::warn!("[Battery] USB query failed: {e:?} — skipping.");
                continue;
            }
            Err(e) => {
                log::warn!("[Battery] spawn_blocking panicked: {e:?} — skipping.");
                continue;
            }
        };

        // Always push to tray so the icon stays current.
        let (pct, charging) = battery_to_pct(&new_state);
        tx.send(TrayUpdate {
            device_name: cobra_name.clone(),
            percentage: pct,
            is_charging: charging,
        })
        .ok();

        // Only emit the D-Bus signal when the state actually changes.
        if last_emitted.as_ref() == Some(&new_state) {
            log::debug!("[Battery] No change ({new_state:?}), skipping D-Bus signal.");
            continue;
        }
        last_emitted = Some(new_state.clone());

        let state_json =
            serde_json::to_string(&new_state).expect("BatteryState serialisation should not fail");

        let iface_ref = conn
            .object_server()
            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
            .await
            .expect("DeviceManager interface must be registered");

        {
            let mut iface = iface_ref.get_mut().await;
            iface.update_battery("cobra-pro", new_state.clone());
        }

        DeviceManager::battery_changed(iface_ref.signal_emitter(), "cobra-pro", &state_json)
            .await
            .ok();

        println!("[Battery] BatteryChanged: cobra-pro → {new_state:?}");
    }
}

fn main() {
    // Initialise structured logging; RUST_LOG controls verbosity (default: info).
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // GTK must be initialised on the main thread before any GLib/AppIndicator
    // calls.  The entire GTK event loop runs here; tokio lives on a worker thread.
    gtk::init().expect("GTK initialisation failed");

    let (tx, rx) = std::sync::mpsc::channel::<TrayUpdate>();

    // Spawn the tokio async runtime on a dedicated background thread so that
    // the blocking GTK main loop can own the main thread exclusively.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime creation failed");
        rt.block_on(run_daemon(tx));
    });

    // Register the AppIndicator tray and set up the GLib polling timeout.
    tray::start_tray(rx);

    // Hand control to GTK — this blocks until the process is killed.
    gtk::main();
}
