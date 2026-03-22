mod device_manager;
mod razer_protocol;
mod tray;
mod usb_backend;

use device_manager::DeviceManager;
use synaptix_protocol::{registry::get_device_profile, BatteryState, RazerDevice, RazerProductId};
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
    let cobra_pid = RazerProductId::CobraProWireless.usb_pid();
    let txn_id = razer_protocol::TRANSACTION_ID_COBRA;
    let wait_us = razer_protocol::WAIT_NEW_RECEIVER_US;

    let cobra_name = get_device_profile(cobra_pid)
        .map(|p| p.name)
        .unwrap_or_else(|| "Razer Cobra Pro (Wireless)".to_string());

    // Query real battery state on startup so the tray shows real data immediately.
    let initial_battery =
        tokio::task::spawn_blocking(move || usb_backend::query_battery(cobra_pid, txn_id, wait_us))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                eprintln!("[Battery] Startup query failed — defaulting to 0 %.");
                BatteryState::Discharging(0)
            });

    println!("[Battery] Startup state: {initial_battery:?}");

    // Push the initial reading to the tray immediately.
    let (pct, charging) = battery_to_pct(&initial_battery);
    tx.send(TrayUpdate {
        device_name: cobra_name.clone(),
        percentage: pct,
        is_charging: charging,
    })
    .ok();

    let mut manager = DeviceManager::new();
    manager.add_device(
        "cobra-pro".to_string(),
        RazerDevice {
            name: cobra_name.clone(),
            product_id: RazerProductId::CobraProWireless,
            battery_state: initial_battery.clone(),
        },
    );

    let conn = match zbus::connection::Builder::session()
        .and_then(|b| b.name("org.synaptix.Daemon"))
        .and_then(|b| b.serve_at("/org/synaptix/Daemon", manager))
    {
        Ok(builder) => match builder.build().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Daemon] D-Bus connection failed: {e:?}");
                return;
            }
        },
        Err(e) => {
            eprintln!("[Daemon] D-Bus builder failed: {e:?}");
            return;
        }
    };

    println!("Synaptix Daemon running on org.synaptix.Daemon at /org/synaptix/Daemon");

    let mut last_emitted: Option<BatteryState> = Some(initial_battery);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        let new_state = match tokio::task::spawn_blocking(move || {
            usb_backend::query_battery(cobra_pid, txn_id, wait_us)
        })
        .await
        {
            Ok(Ok(state)) => state,
            Ok(Err(e)) => {
                eprintln!("[Battery] USB query failed: {e:?} — skipping.");
                continue;
            }
            Err(e) => {
                eprintln!("[Battery] spawn_blocking panicked: {e:?} — skipping.");
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
            println!("[Battery] No change ({new_state:?}), skipping D-Bus signal.");
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
