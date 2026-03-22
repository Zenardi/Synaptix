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
    // level actually changes so the hardware microcontroller can idle between
    // polls without being woken by unnecessary USB traffic.
    let mut level: u8 = 75;
    let mut last_emitted: Option<u8> = None;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        // TODO: replace with a real USB battery query via usb_backend.
        level = level.saturating_sub(1);

        // Skip the update if nothing changed since the last emission.
        if last_emitted == Some(level) {
            println!("[Battery] No change ({level}%), skipping D-Bus signal.");
            continue;
        }
        last_emitted = Some(level);

        let new_state = BatteryState::Discharging(level);
        let state_json = serde_json::to_string(&new_state)
            .expect("BatteryState serialisation should not fail");

        let iface_ref = conn
            .object_server()
            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
            .await
            .expect("DeviceManager interface must be registered");

        {
            let mut iface = iface_ref.get_mut().await;
            iface.update_battery("cobra-pro", new_state);
        }

        DeviceManager::battery_changed(
            &iface_ref.signal_emitter(),
            "cobra-pro",
            &state_json,
        )
        .await
        .ok();

        println!("[Battery] BatteryChanged: cobra-pro → {level}%");
    }
}
