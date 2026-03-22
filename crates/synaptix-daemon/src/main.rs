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

/// Extracts the percentage from any `BatteryState` variant.
fn state_pct(state: &BatteryState) -> u8 {
    match state {
        BatteryState::Charging(n) | BatteryState::Discharging(n) => *n,
        BatteryState::Full => 100,
    }
}


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

    // ── Connection-watch task (every 1 s) ────────────────────────────────────
    // Cheap USB descriptor scan — no device open, no I/O. Fires a D-Bus signal
    // the moment the user switches from dongle to cable (or vice versa) so the
    // React UI updates within one second without a reload.
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

    // Track cable presence separately: when dongle+cable are both connected,
    // the connection type stays "Dongle" but charging state must update quickly.
    let watch_cable_init = if matches!(watch_conn, ConnectionType::Dongle) {
        tokio::task::spawn_blocking(|| {
            usb_backend::detect_connected_pid(&[usb_backend::COBRA_PRO_WIRED_PID]).is_some()
        })
        .await
        .unwrap_or(false)
    } else {
        false
    };

    tokio::spawn(async move {
        let mut watch_cable_present = watch_cable_init;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            let (new_pid, new_product_id, new_conn, new_name, new_cable) =
                tokio::task::spawn_blocking(|| {
                    let (pid, prod, conn, name) = detect_cobra_pro();
                    // Detect cable separately — when both dongle + cable are
                    // present detect_cobra_pro returns Dongle, but we still need
                    // to know the cable appeared/disappeared for charging updates.
                    let cable = matches!(conn, ConnectionType::Dongle)
                        && usb_backend::detect_connected_pid(&[usb_backend::COBRA_PRO_WIRED_PID])
                            .is_some();
                    (pid, prod, conn, name, cable)
                })
                .await
                .unwrap_or_else(|_| {
                    (watch_pid, watch_product_id.clone(), watch_conn.clone(), String::new(), watch_cable_present)
                });

            let conn_changed = new_pid != watch_pid || new_conn != watch_conn;
            let cable_changed = new_cable != watch_cable_present;

            if !conn_changed && !cable_changed {
                continue;
            }

            watch_pid = new_pid;
            watch_product_id = new_product_id.clone();
            watch_conn = new_conn.clone();
            watch_cable_present = new_cable;

            // Update shared state so the 60s poll uses the right PID/conn type.
            {
                let mut s = shared_watch.lock().await;
                *s = (new_pid, new_name.clone(), new_conn.clone());
            }

            // Emit ConnectionChanged only when the actual connection type switched.
            if conn_changed {
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

                log::info!(
                    "[Detect] Connection changed to {} (PID 0x{new_pid:04X})",
                    new_conn.label()
                );
            } else {
                log::info!("[Detect] Cable {} while {}", if new_cable { "plugged in" } else { "unplugged" }, new_conn.label());
            }

            // Immediately update battery after any connection or cable change.
            // Wired: Wait 200 ms for USB enumeration, then query with retries.
            // Dongle: query immediately (cable-presence check in query_battery handles charging).
            // Bluetooth: no USB interface — skip.
            let fresh_state_opt: Option<BatteryState> = match &new_conn {
                ConnectionType::Wired => {
                    // Short pause so the kernel HID driver finishes binding before
                    // we try to send a control transfer.
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                    let mut wired_state: Option<BatteryState> = None;
                    for attempt in 1..=3u8 {
                        let pid_c = new_pid;
                        let result = tokio::task::spawn_blocking(move || {
                            usb_backend::query_battery(pid_c, txn_id, wait_us, &ConnectionType::Wired)
                        })
                        .await
                        .ok()
                        .and_then(|r| r.ok());

                        match result {
                            Some(BatteryState::Full) => {
                                wired_state = Some(BatteryState::Full);
                                break;
                            }
                            Some(BatteryState::Charging(n)) if n > 0 => {
                                wired_state = Some(BatteryState::Charging(n));
                                break;
                            }
                            Some(BatteryState::Discharging(n)) if n > 0 => {
                                // Wired = always charging; override variant.
                                wired_state = Some(BatteryState::Charging(n));
                                break;
                            }
                            _ => {
                                log::warn!("[Battery] Wired query attempt {attempt} returned 0 or failed — retrying…");
                                tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                            }
                        }
                    }

                    if wired_state.is_none() {
                        // USB returned 0 on all attempts (device still initialising
                        // or genuinely 0%). Fall back to the last known level so
                        // we don't overwrite a valid wireless reading with 0%.
                        if let Ok(bat_ref) = conn_watch
                            .object_server()
                            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
                            .await
                        {
                            let iface = bat_ref.get().await;
                            wired_state = iface.devices.get("cobra-pro").and_then(|d| {
                                let pct = match &d.battery_state {
                                    BatteryState::Charging(n) | BatteryState::Discharging(n) => *n,
                                    BatteryState::Full => 100,
                                };
                                // Only emit if we have a meaningful level.
                                if pct > 0 {
                                    Some(if pct >= 100 { BatteryState::Full } else { BatteryState::Charging(pct) })
                                } else {
                                    // No valid level anywhere — skip the battery emit.
                                    // The ⚡ Charging badge still shows via connection_type.
                                    log::warn!("[Battery] No valid level for Wired device; skipping battery emit.");
                                    None
                                }
                            });
                        }
                    }

                    wired_state
                }
                ConnectionType::Dongle => {
                    let conn_c = new_conn.clone();
                    tokio::task::spawn_blocking(move || {
                        usb_backend::query_battery(new_pid, txn_id, wait_us, &conn_c)
                    })
                    .await
                    .ok()
                    .and_then(|r| r.ok())
                }
                ConnectionType::Bluetooth => None,
            };

            if let Some(fresh_state) = fresh_state_opt {
                // Sanity check: if the new reading is 0% but the DeviceManager
                // holds a valid (> 5%) last known level, it's a bad USB read —
                // discard it. A real battery cannot drop from 75% to 0% instantly.
                let should_emit = {
                    let new_pct = state_pct(&fresh_state);
                    if new_pct == 0 {
                        if let Ok(bat_ref) = conn_watch
                            .object_server()
                            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
                            .await
                        {
                            let iface = bat_ref.get().await;
                            let last_pct = iface
                                .devices
                                .get("cobra-pro")
                                .map(|d| state_pct(&d.battery_state))
                                .unwrap_or(0);
                            if last_pct > 5 {
                                log::warn!(
                                    "[Battery] Discarding suspicious 0% post-reconnect read (last known: {last_pct}%)"
                                );
                                false
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                };

                if should_emit {
                    let state_json = serde_json::to_string(&fresh_state)
                        .unwrap_or_else(|_| "\"Discharging\"".to_string());
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

        // Sanity check: reject a 0% reading if the last known level was healthy
        // (> 5%). When the USB cable is plugged in alongside the dongle, the Cobra
        // Pro stops responding to dongle battery queries and returns 0. A real
        // battery cannot drain from > 5% to 0% between two consecutive 60-second
        // polls; any such reading is a bad USB transfer — skip it entirely.
        let last_pct = last_emitted.as_ref().map(state_pct).unwrap_or(0);
        let new_pct = state_pct(&new_state);
        if new_pct == 0 && last_pct > 5 {
            log::warn!(
                "[Battery] Discarding suspicious 0% reading (last known: {last_pct}%) — bad USB read."
            );
            continue;
        }

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
