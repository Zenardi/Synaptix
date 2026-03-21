use std::collections::HashMap;
#[cfg(not(test))]
use synaptix_protocol::{BatteryState, RazerDevice};
#[cfg(test)]
use synaptix_protocol::{BatteryState, RazerDevice, RazerProductId};

pub struct DeviceManager {
    devices: HashMap<String, RazerDevice>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn add_device(&mut self, id: String, device: RazerDevice) {
        self.devices.insert(id, device);
    }

    pub fn get_device(&self, id: &str) -> Option<&RazerDevice> {
        self.devices.get(id)
    }

    pub fn get_all_devices(&self) -> Vec<&RazerDevice> {
        self.devices.values().collect()
    }

    pub fn update_battery(&mut self, id: &str, state: BatteryState) {
        if let Some(device) = self.devices.get_mut(id) {
            device.battery_state = state;
        }
    }
}

/// D-Bus interface: exposes device state on `org.synaptix.Daemon`.
///
/// `GetDevices` returns a JSON-serialised array of `RazerDevice` objects so
/// that any D-Bus consumer (including our Tauri IPC layer) can deserialise
/// them without coupling to zbus-specific types.
#[zbus::interface(name = "org.synaptix.Daemon")]
impl DeviceManager {
    fn get_devices(&self) -> Vec<String> {
        self.devices
            .values()
            .filter_map(|d| serde_json::to_string(d).ok())
            .collect()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn mock_device() -> RazerDevice {
        RazerDevice {
            name: "Razer DeathAdder V2 Pro".to_string(),
            product_id: RazerProductId::DeathAdderV2Pro,
            battery_state: BatteryState::Discharging(75),
        }
    }

    #[test]
    fn test_add_and_retrieve_device() {
        let mut manager = DeviceManager::new();
        let device = mock_device();

        manager.add_device("da-v2-pro".to_string(), device.clone());

        let retrieved = manager.get_device("da-v2-pro").expect("device should exist");
        assert_eq!(retrieved.name, device.name);
        assert_eq!(retrieved.product_id, device.product_id);
        assert_eq!(retrieved.battery_state, device.battery_state);
    }

    #[test]
    fn test_update_battery_state() {
        let mut manager = DeviceManager::new();
        manager.add_device("da-v2-pro".to_string(), mock_device());

        manager.update_battery("da-v2-pro", BatteryState::Charging(80));

        let device = manager.get_device("da-v2-pro").expect("device should exist");
        assert_eq!(device.battery_state, BatteryState::Charging(80));
    }
}
