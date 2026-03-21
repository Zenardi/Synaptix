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

    let _conn = zbus::connection::Builder::session()?
        .name("org.synaptix.Daemon")?
        .serve_at("/org/synaptix/Daemon", manager)?
        .build()
        .await?;

    println!("Synaptix Daemon running on org.synaptix.Daemon at /org/synaptix/Daemon");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
