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

// PIDs for the Cobra Pro, dongle-first: when both are plugged in the dongle
// is the active gaming connection and the cable is just charging.
const COBRA_PRO_PIDS: &[(u16, ConnectionType)] = &[
    (0x00B0, ConnectionType::Dongle),  // HyperSpeed dongle (preferred)
    (0x00AF, ConnectionType::Wired),   // USB cable (charging / wired-only mode)
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
    let (initial_pid, initial_product_id, initial_conn, initial_name) =
        tokio::task::spawn_blocking(detect_cobra_pro)
            .await
            .unwrap_or_else(|_| (0x00B0, RazerProductId::CobraProWireless, ConnectionType::Bluetooth, "Razer Cobra Pro".to_string()));

    log::info!("[Detect] Cobra Pro on USB: PID=0x{initial_pid:04X} connection={}", initial_conn.label());

    // Shared state: the watch task owns mutation; the battery loop reads it.
    let shared: std::sync::Arc<tokio::sync::Mutex<(u16, String, ConnectionType)>> =
        std::sync::Arc::new(tokio::sync::Mutex::new((initial_pid, initial_name.clone(), initial_conn.clone())));

    let cobra_pid = initial_pid;
    let cobra_name = initial_name.clone();

    // Query real battery state on startup so the tray shows real data immediately.
    // Retry up to 3 times (500ms apart) in case the USB device isn't ready yet.
    let initial_battery = {
        let pid = cobra_pid;
        let conn = initial_conn.clone();
        let mut attempt = 0u8;
        loop {
            let pid_c = pid;
            let conn_c = conn.clone();
            let result = tokio::task::spawn_blocking(move || {
                usb_backend::query_battery(pid_c, txn_id, wait_us, &conn_c)
            })
            .await
            .ok()
            .and_then(|r| r.ok());

            match result {
                Some(state) => break state,
                None => {
                    attempt += 1;
                    if attempt >= 3 {
                        log::warn!("[Battery] Startup query failed after 3 attempts — defaulting to unknown.");
                        break BatteryState::Discharging(0);
                    }
                    log::warn!("[Battery] Startup query attempt {attempt} failed, retrying in 500ms…");
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
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
    let cobra_capabilities = get_device_profile(initial_pid)
        .map(|p| p.capabilities)
        .unwrap_or_default();
    manager.add_device(
        "cobra-pro".to_string(),
        RazerDevice {
            name: initial_name.clone(),
            product_id: initial_product_id,
            battery_state: initial_battery.clone(),
            capabilities: cobra_capabilities,
            connection_type: initial_conn,
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

    // ── Connection-watch task (every 3 s) ────────────────────────────────────
    // Cheap USB descriptor scan — no device open, no I/O. Fires a D-Bus signal
    // the moment the user switches from dongle to cable (or vice versa) so the
    // React UI updates within seconds without a reload.
    let conn_watch = conn.clone();
    let shared_watch = std::sync::Arc::clone(&shared);
    let mut watch_pid = initial_pid;
    let mut watch_conn = {
        // Reconstruct from initial detection for the watch loop's bookkeeping.
        let (_, _, c, _) = tokio::task::spawn_blocking(detect_cobra_pro)
            .await
            .unwrap_or_else(|_| (initial_pid, RazerProductId::CobraProWireless, ConnectionType::Bluetooth, initial_name.clone()));
        c
    };
    let mut watch_product_id = {
        if initial_pid == 0x00AF { RazerProductId::CobraProWired } else { RazerProductId::CobraProWireless }
    };

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            let (new_pid, new_product_id, new_conn, new_name) =
                tokio::task::spawn_blocking(detect_cobra_pro)
                    .await
                    .unwrap_or_else(|_| {
                        (watch_pid, watch_product_id.clone(), watch_conn.clone(), String::new())
                    });

            if new_pid == watch_pid && new_conn == watch_conn {
                continue;
            }

            log::info!(
                "[Detect] Connection changed: {} → {} (PID 0x{:04X} → 0x{:04X})",
                watch_conn.label(), new_conn.label(), watch_pid, new_pid
            );

            watch_pid = new_pid;
            watch_product_id = new_product_id.clone();
            watch_conn = new_conn.clone();

            // Update shared state so the battery loop uses the right PID + connection type.
            {
                let mut s = shared_watch.lock().await;
                *s = (new_pid, new_name.clone(), new_conn.clone());
            }

            let Ok(iface_ref) = conn_watch
                .object_server()
                .interface::<_, DeviceManager>("/org/synaptix/Daemon")
                .await
            else {
                continue;
            };

            {
                let mut iface = iface_ref.get_mut().await;
                iface.update_connection(
                    "cobra-pro",
                    new_name,
                    new_product_id,
                    new_conn.clone(),
                );
            }

            let conn_json = serde_json::to_string(&new_conn)
                .unwrap_or_else(|_| "\"Bluetooth\"".to_string());
            DeviceManager::connection_changed(
                iface_ref.signal_emitter(),
                "cobra-pro",
                &conn_json,
            )
            .await
            .ok();

            // Immediately re-query battery after a connection change so the UI
            // reflects the new charging state without waiting 60 s.
            if new_pid != 0 {
                let conn_for_query = new_conn.clone();
                if let Ok(Ok(fresh_state)) = tokio::task::spawn_blocking(move || {
                    usb_backend::query_battery(new_pid, txn_id, wait_us, &conn_for_query)
                })
                .await
                {
                    let state_json = serde_json::to_string(&fresh_state)
                        .unwrap_or_else(|_| "\"Discharging\"".to_string());
                    // Re-acquire the interface ref for the battery signal.
                    if let Ok(bat_iface_ref) = conn_watch
                        .object_server()
                        .interface::<_, DeviceManager>("/org/synaptix/Daemon")
                        .await
                    {
                        {
                            let mut iface = bat_iface_ref.get_mut().await;
                            iface.update_battery("cobra-pro", fresh_state.clone());
                        }
                        DeviceManager::battery_changed(
                            bat_iface_ref.signal_emitter(),
                            "cobra-pro",
                            &state_json,
                        )
                        .await
                        .ok();
                        log::info!("[Battery] Post-reconnect state: {fresh_state:?}");
                    }
                }
            }
        }
    });

    // ── Battery-poll loop (every 60 s) ────────────────────────────────────────
    let mut last_emitted: Option<BatteryState> = Some(initial_battery);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        // Read the current PID, name, and connection type from shared state (updated by watch task).
        let (pid, current_name, current_conn) = {
            let s = shared.lock().await;
            s.clone()
        };

        let new_state = match tokio::task::spawn_blocking(move || {
            usb_backend::query_battery(pid, txn_id, wait_us, &current_conn)
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
            device_name: current_name,
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
