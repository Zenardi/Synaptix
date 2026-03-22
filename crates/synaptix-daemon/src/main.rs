mod device_manager;
mod razer_protocol;
mod usb_backend;

use device_manager::DeviceManager;
use synaptix_protocol::{registry::get_device_profile, BatteryState, RazerDevice, RazerProductId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cobra_pid = RazerProductId::CobraProWireless.usb_pid();
    let txn_id = razer_protocol::TRANSACTION_ID_COBRA;

    let cobra_name = get_device_profile(cobra_pid)
        .map(|p| p.name)
        .unwrap_or_else(|| "Razer Cobra Pro (Wireless)".to_string());

    // Query real battery state immediately on startup so the UI never shows
    // a stale placeholder value.
    let initial_battery =
        tokio::task::spawn_blocking(move || usb_backend::query_battery(cobra_pid, txn_id))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_else(|| {
                eprintln!("[Battery] Startup query failed — defaulting to Unknown.");
                BatteryState::Discharging(0)
            });

    println!("[Battery] Startup state: {initial_battery:?}");

    let mut manager = DeviceManager::new();
    manager.add_device(
        "cobra-pro".to_string(),
        RazerDevice {
            name: cobra_name,
            product_id: RazerProductId::CobraProWireless,
            battery_state: initial_battery.clone(),
        },
    );

    let conn = zbus::connection::Builder::session()?
        .name("org.synaptix.Daemon")?
        .serve_at("/org/synaptix/Daemon", manager)?
        .build()
        .await?;

    println!("Synaptix Daemon running on org.synaptix.Daemon at /org/synaptix/Daemon");

    // Poll battery state once per minute.  The signal is only emitted when the
    // state actually changes so the hardware microcontroller can idle between
    // polls without being woken by unnecessary USB traffic.
    let mut last_emitted: Option<BatteryState> = Some(initial_battery);
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        // Query real battery state from hardware inside a blocking thread so
        // the async runtime is not stalled by synchronous USB I/O.
        let new_state = match tokio::task::spawn_blocking(move || {
            usb_backend::query_battery(cobra_pid, txn_id)
        })
        .await
        {
            Ok(Ok(state)) => state,
            Ok(Err(e)) => {
                eprintln!("[Battery] USB query failed: {e:?} — skipping this cycle.");
                continue;
            }
            Err(e) => {
                eprintln!("[Battery] spawn_blocking panicked: {e:?} — skipping this cycle.");
                continue;
            }
        };

        // Skip the update if nothing changed since the last emission.
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
