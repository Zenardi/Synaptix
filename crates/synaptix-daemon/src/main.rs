mod device_manager;

use device_manager::DeviceManager;
use synaptix_protocol::{BatteryState, RazerDevice, RazerProductId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = DeviceManager::new();

    // Seed a mock device so the daemon always has something to broadcast.
    manager.add_device(
        "da-v2-pro".to_string(),
        RazerDevice {
            name: "Razer DeathAdder V2 Pro".to_string(),
            product_id: RazerProductId::DeathAdderV2Pro,
            battery_state: BatteryState::Discharging(75),
        },
    );

    let conn = zbus::connection::Builder::session()?
        .name("org.synaptix.Daemon")?
        .serve_at("/org/synaptix/Daemon", manager)?
        .build()
        .await?;

    println!("Synaptix Daemon running on org.synaptix.Daemon at /org/synaptix/Daemon");

    // Simulate battery drain: decrement level every 10 s and broadcast the
    // BatteryChanged signal so connected UIs update in real-time.
    let mut level: u8 = 75;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        level = level.saturating_sub(1);

        let new_state = BatteryState::Discharging(level);
        let state_json = serde_json::to_string(&new_state)
            .expect("BatteryState serialisation should not fail");

        // Update internal state through the registered interface.
        let iface_ref = conn
            .object_server()
            .interface::<_, DeviceManager>("/org/synaptix/Daemon")
            .await
            .expect("DeviceManager interface must be registered");

        {
            let mut iface = iface_ref.get_mut().await;
            iface.update_battery("da-v2-pro", new_state);
        }

        // Emit the D-Bus signal so all subscribers are notified.
        DeviceManager::battery_changed(
            &iface_ref.signal_emitter(),
            "da-v2-pro",
            &state_json,
        )
        .await
        .ok();

        println!("BatteryChanged: da-v2-pro → {level}%");
    }
}
