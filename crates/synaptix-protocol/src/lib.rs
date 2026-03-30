pub mod registry;

use serde::{Deserialize, Serialize};

/// All known Razer wireless/wired device product IDs (VID 0x1532).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RazerProductId {
    // Mice
    DeathAdderV2Pro,       // 0x007C
    MambaWireless,         // 0x0073
    ViperUltimateWired,    // 0x007A
    ViperUltimateWireless, // 0x007B
    BasiliskUltimate,      // 0x0085
    NagaPro,               // 0x008F
    CobraProWired,         // 0x00AF
    CobraProWireless,      // 0x00B0
    // Headsets
    KrakenUltimate, // 0x0527
    KrakenKittyV2,  // 0x0560 (Razer Kraken Kitty V2)
    KrakenV4Pro,    // 0x0568
    // Keyboards
    BlackWidowV3Pro,                    // 0x025A
    BlackWidowV3MiniHyperSpeedWired,    // 0x0258
    BlackWidowV3MiniHyperSpeedWireless, // 0x0271
    // Catch-all for devices not yet enumerated
    Unknown(u16),
}

/// A lighting effect to apply to a device's RGB zones.
///
/// Wire format (serde defaults):
///   Static:    `{ "Static": [r, g, b] }`
///   Breathing: `{ "Breathing": [r, g, b] }`
///   Spectrum:  `"Spectrum"` (unit variant → plain string)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightingEffect {
    /// All LEDs set to a single static RGB colour.
    Static([u8; 3]),
    /// Single-colour pulsing breathing effect.
    Breathing([u8; 3]),
    /// Automatic full-spectrum colour cycling.
    Spectrum,
}

/// Persistent user preferences for a single device.
/// Stored in `~/.config/synaptix/devices.json` and auto-applied on daemon start.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeviceSettings {
    pub lighting: Option<LightingEffect>,
    pub dpi: Option<u16>,
}

/// A sensor (optical / laser) configuration command for a mouse.
///
/// Wire format (serde defaults):
///   SetDpi: `{ "SetDpi": { "x": 800, "y": 800 } }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensorCommand {
    /// Set the X and Y DPI independently.
    /// Valid range: 100–45 000 (device-specific maximum applies at the hardware level).
    SetDpi { x: u16, y: u16 },
}

/// Audio / haptic configuration commands for headset devices.
///
/// These map directly to USB HID payloads sent to the headset endpoint.
/// Command constants are based on the historical Kraken V3 HyperSense protocol;
/// **Wireshark verification required** for Kraken V4 Pro (PID 0x0568).
///
/// Wire format (serde defaults):
///   SetSidetone:        `{ "SetSidetone": { "level": 50 } }`
///   SetHapticIntensity: `{ "SetHapticIntensity": { "level": 75 } }`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCommand {
    /// Set sidetone volume — hear your own voice in the headset ear cups.
    /// Valid range: 0 (silent) – 100 (full volume).
    SetSidetone { level: u8 },
    /// Set haptic feedback intensity on HyperSense-equipped headsets.
    /// Valid range: 0 (disabled) – 100 (maximum intensity).
    SetHapticIntensity { level: u8 },
}

/// The battery / charging state reported by `razer.device.power`.
///
/// `Charging(u8)` and `Discharging(u8)` carry the current charge level (0–100).
/// `Full` is reported when the device is on the charger and fully charged.
/// `Unknown` is used when the battery level cannot be determined (e.g. the USB
/// query is unsupported or failed) — the UI should display "?" in this case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatteryState {
    Charging(u8),
    Discharging(u8),
    Full,
    Unknown,
}

impl RazerProductId {
    /// Returns the USB product ID (PID) for this device (VID is always `0x1532`).
    pub fn usb_pid(&self) -> u16 {
        match self {
            RazerProductId::DeathAdderV2Pro => 0x007C,
            RazerProductId::MambaWireless => 0x0073,
            RazerProductId::ViperUltimateWired => 0x007A,
            RazerProductId::ViperUltimateWireless => 0x007B,
            RazerProductId::BasiliskUltimate => 0x0085,
            RazerProductId::NagaPro => 0x008F,
            RazerProductId::CobraProWired => 0x00AF,
            RazerProductId::CobraProWireless => 0x00B0,
            RazerProductId::KrakenUltimate => 0x0527,
            RazerProductId::KrakenKittyV2 => 0x0560,
            RazerProductId::KrakenV4Pro => 0x0568,
            RazerProductId::BlackWidowV3Pro => 0x025A,
            RazerProductId::BlackWidowV3MiniHyperSpeedWired => 0x0258,
            RazerProductId::BlackWidowV3MiniHyperSpeedWireless => 0x0271,
            RazerProductId::Unknown(pid) => *pid,
        }
    }
}

impl BatteryState {
    /// Returns the charge level percentage if one is available.
    pub fn level(&self) -> Option<u8> {
        match self {
            BatteryState::Charging(lvl) | BatteryState::Discharging(lvl) => Some(*lvl),
            BatteryState::Full | BatteryState::Unknown => None,
        }
    }
}

/// How the device is physically connected to the host.
///
/// `Bluetooth` is reserved for future use — Bluetooth HID devices do not
/// appear in `rusb`; when the daemon detects neither wired nor dongle PID on
/// the USB bus it leaves the device as `Bluetooth`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectionType {
    Wired,
    Dongle,
    #[default]
    Bluetooth,
}

impl ConnectionType {
    /// Human-readable label shown in the UI.
    pub fn label(&self) -> &'static str {
        match self {
            ConnectionType::Wired => "Wired",
            ConnectionType::Dongle => "USB Dongle",
            ConnectionType::Bluetooth => "Bluetooth",
        }
    }
}

/// A Razer device as represented on the D-Bus interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RazerDevice {
    pub name: String,
    pub product_id: RazerProductId,
    pub battery_state: BatteryState,
    pub capabilities: Vec<registry::DeviceCapability>,
    #[serde(default)]
    pub connection_type: ConnectionType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_serialization() {
        let device = RazerDevice {
            name: "Razer DeathAdder V2 Pro".to_string(),
            product_id: RazerProductId::DeathAdderV2Pro,
            battery_state: BatteryState::Discharging(75),
            capabilities: vec![],
            connection_type: ConnectionType::Wired,
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

    #[test]
    fn test_usb_pid_blackwidow_v3_mini_hyperspeed_wired() {
        assert_eq!(
            RazerProductId::BlackWidowV3MiniHyperSpeedWired.usb_pid(),
            0x0258,
            "Wired PID must be 0x0258 per razerkbd_driver.h"
        );
    }

    #[test]
    fn test_usb_pid_blackwidow_v3_mini_hyperspeed_wireless() {
        assert_eq!(
            RazerProductId::BlackWidowV3MiniHyperSpeedWireless.usb_pid(),
            0x0271,
            "Wireless PID must be 0x0271 per razerkbd_driver.h"
        );
    }

    #[test]
    fn test_blackwidow_v3_mini_enum_variants_are_distinct() {
        assert_ne!(
            RazerProductId::BlackWidowV3MiniHyperSpeedWired,
            RazerProductId::BlackWidowV3MiniHyperSpeedWireless
        );
        assert_ne!(
            RazerProductId::BlackWidowV3MiniHyperSpeedWired.usb_pid(),
            RazerProductId::BlackWidowV3MiniHyperSpeedWireless.usb_pid()
        );
    }

    #[test]
    fn test_blackwidow_v3_mini_serialization_roundtrip() {
        let device = RazerDevice {
            name: "Razer BlackWidow V3 Mini HyperSpeed (Wireless)".to_string(),
            product_id: RazerProductId::BlackWidowV3MiniHyperSpeedWireless,
            battery_state: BatteryState::Discharging(82),
            capabilities: vec![],
            connection_type: ConnectionType::Dongle,
        };

        let json = serde_json::to_string(&device).expect("serialization failed");
        let restored: RazerDevice = serde_json::from_str(&json).expect("deserialization failed");

        assert_eq!(
            restored.product_id,
            RazerProductId::BlackWidowV3MiniHyperSpeedWireless
        );
        assert_eq!(restored.battery_state, BatteryState::Discharging(82));
        assert_eq!(restored.connection_type, ConnectionType::Dongle);
    }

    #[test]
    fn test_audio_command_serialization() {
        let sidetone = AudioCommand::SetSidetone { level: 50 };
        let haptic = AudioCommand::SetHapticIntensity { level: 75 };

        let json_st = serde_json::to_string(&sidetone).expect("sidetone serialization failed");
        let json_hp = serde_json::to_string(&haptic).expect("haptic serialization failed");

        let restored_st: AudioCommand =
            serde_json::from_str(&json_st).expect("sidetone deserialization failed");
        let restored_hp: AudioCommand =
            serde_json::from_str(&json_hp).expect("haptic deserialization failed");

        assert_eq!(restored_st, sidetone);
        assert_eq!(restored_hp, haptic);

        // Verify level round-trips correctly
        assert!(matches!(
            restored_st,
            AudioCommand::SetSidetone { level: 50 }
        ));
        assert!(matches!(
            restored_hp,
            AudioCommand::SetHapticIntensity { level: 75 }
        ));
    }
}
