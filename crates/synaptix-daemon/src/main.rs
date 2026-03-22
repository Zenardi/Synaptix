mod device_manager;
mod razer_protocol;
mod usb_backend;

use device_manager::DeviceManager;
use synaptix_protocol::{BatteryState, RazerDevice, RazerProductId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = DeviceManager::new();

    // Seed the Cobra Pro as the active device.
    manager.add_device(
        "cobra-pro".to_string(),
        RazerDevice {
            name: "Razer Cobra Pro".to_string(),
            product_id: RazerProductId::CobraProWireless,
            battery_state: BatteryState::Discharging(75),
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
    let pid = RazerProductId::CobraProWireless.usb_pid();
    let txn_id = razer_protocol::TRANSACTION_ID_COBRA;
    let mut last_emitted: Option<BatteryState> = None;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        // Query real battery state from hardware inside a blocking thread so
        // the async runtime is not stalled by synchronous USB I/O.
        let new_state = match tokio::task::spawn_blocking(move || {
            usb_backend::query_battery(pid, txn_id)
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
