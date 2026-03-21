use serde::{Deserialize, Serialize};

/// All known Razer wireless/wired device product IDs (VID 0x1532).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RazerProductId {
    // Mice
    DeathAdderV2Pro,          // 0x007C
    MambaWireless,            // 0x0073
    ViperUltimate,            // 0x007A
    BasiliskUltimate,         // 0x0085
    NagaPro,                  // 0x008F
    // Headsets
    KrakenUltimate,           // 0x0527
    KrakenV3HyperSense,       // 0x0560
    // Keyboards
    BlackWidowV3Pro,          // 0x025A
    // Catch-all for devices not yet enumerated
    Unknown(u16),
}

/// The battery / charging state reported by `razer.device.power`.
///
/// `Charging(u8)` and `Discharging(u8)` carry the current charge level (0–100).
/// `Full` is reported when the device is on the charger and fully charged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryState {
    Charging(u8),
    Discharging(u8),
    Full,
}

impl BatteryState {
    /// Returns the charge level percentage if one is available.
    pub fn level(&self) -> Option<u8> {
        match self {
            BatteryState::Charging(lvl) | BatteryState::Discharging(lvl) => Some(*lvl),
            BatteryState::Full => None,
        }
    }
}

/// A Razer device as represented on the D-Bus interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RazerDevice {
    pub name: String,
    pub product_id: RazerProductId,
    pub battery_state: BatteryState,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_device_serialization() {
        let device = RazerDevice {
            name: "Razer DeathAdder V2 Pro".to_string(),
            product_id: RazerProductId::DeathAdderV2Pro,
            battery_state: BatteryState::Discharging(75),
        };

        let json = serde_json::to_string(&device).expect("serialization failed");
        let restored: RazerDevice = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(restored.name, device.name);
        assert_eq!(restored.product_id, device.product_id);
        assert_eq!(restored.battery_state, device.battery_state);
    }

    #[test]
    fn test_battery_state_transitions() {
        let charging = BatteryState::Charging(42);
        let discharging = BatteryState::Discharging(80);
        let full = BatteryState::Full;

        // Charging carries a percentage too (battery level while on the charger)
        assert_eq!(charging, BatteryState::Charging(42));
        assert_ne!(charging, discharging);
        assert_eq!(full, BatteryState::Full);

        // Verify level extraction
        assert_eq!(charging.level(), Some(42));
        assert_eq!(discharging.level(), Some(80));
        assert_eq!(full.level(), None);
    }
}
