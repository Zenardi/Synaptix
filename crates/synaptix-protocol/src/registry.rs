use crate::LightingEffect;
use serde::{Deserialize, Serialize};

/// The class of peripheral.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    Mouse,
    Keyboard,
    Audio,
}

/// Logical capabilities a device exposes beyond basic connectivity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceCapability {
    /// Device supports at least one `LightingEffect`.
    Lighting(LightingEffect),
    /// Device can report battery level and charging status via USB.
    BatteryReporting,
}

/// Static profile for a known Razer device sourced from the USB PID registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Human-readable display name.
    pub name: String,
    /// USB product ID (Razer VID is always `0x1532`).
    pub product_id: u16,
    /// The class of device.
    pub device_type: DeviceType,
    /// Capabilities supported by this device.
    pub capabilities: Vec<DeviceCapability>,
}

/// Looks up a [`DeviceProfile`] by USB product ID (PID).
///
/// Returns `None` when the PID is not in Synaptix's registry.
/// Data sourced from `_reference_openrazer/daemon/openrazer_daemon/hardware/mouse.py`.
pub fn get_device_profile(product_id: u16) -> Option<DeviceProfile> {
    // (name, has_battery_reporting)
    let (name, has_battery): (&str, bool) = match product_id {
        // ── Abyssus family ────────────────────────────────────────────────
        0x0042 => ("Razer Abyssus", false),
        0x0020 => ("Razer Abyssus 1800", false),
        0x005E => ("Razer Abyssus 2000", false),
        0x006B => ("Razer Abyssus Essential", false),
        0x005B => ("Razer Abyssus V2", false),
        0x006A => ("Razer Abyssus Elite D.Va Edition", false),

        // ── Atheris family ────────────────────────────────────────────────
        0x0062 => ("Razer Atheris (Receiver)", true),

        // ── Basilisk family ───────────────────────────────────────────────
        0x0064 => ("Razer Basilisk", false),
        0x0065 => ("Razer Basilisk Essential", false),
        0x0083 => ("Razer Basilisk X HyperSpeed", true),
        0x0085 => ("Razer Basilisk V2", false),
        0x0086 => ("Razer Basilisk Ultimate (Wired)", false),
        0x0088 => ("Razer Basilisk Ultimate (Receiver)", true),
        0x0099 => ("Razer Basilisk V3", false),
        0x00AA => ("Razer Basilisk V3 Pro (Wired)", false),
        0x00AB => ("Razer Basilisk V3 Pro (Wireless)", true),
        0x00B9 => ("Razer Basilisk V3 X HyperSpeed", true),
        0x00CB => ("Razer Basilisk V3 35K", false),
        0x00CC => ("Razer Basilisk V3 Pro 35K (Wired)", false),
        0x00CD => ("Razer Basilisk V3 Pro 35K (Wireless)", true),
        0x00D6 => ("Razer Basilisk V3 Pro 35K Phantom Green (Wired)", false),
        0x00D7 => ("Razer Basilisk V3 Pro 35K Phantom Green (Wireless)", true),

        // ── Cobra family ──────────────────────────────────────────────────
        0x00A3 => ("Razer Cobra", false),
        0x00AF => ("Razer Cobra Pro (Wired)", false),
        0x00B0 => ("Razer Cobra Pro (Wireless)", true),

        // ── DeathAdder family ─────────────────────────────────────────────
        0x0016 => ("Razer DeathAdder 3.5G", false),
        0x0029 => ("Razer DeathAdder 3.5G (Black)", false),
        0x0037 => ("Razer DeathAdder 2013", false),
        0x0038 => ("Razer DeathAdder 1800", false),
        0x0043 => ("Razer DeathAdder Chroma", false),
        0x004F => ("Razer DeathAdder 2000", false),
        0x0054 => ("Razer DeathAdder 3500", false),
        0x005C => ("Razer DeathAdder Elite", false),
        0x006E => ("Razer DeathAdder Essential", false),
        0x0071 => ("Razer DeathAdder Essential (White Edition)", false),
        0x0084 => ("Razer DeathAdder V2", false),
        0x008C => ("Razer DeathAdder V2 Mini", false),
        0x0098 => ("Razer DeathAdder Essential (2021)", false),
        0x009C => ("Razer DeathAdder V2 X HyperSpeed", true),
        0x00A1 => ("Razer DeathAdder V2 Lite", false),
        0x007C => ("Razer DeathAdder V2 Pro (Wired)", false),
        0x007D => ("Razer DeathAdder V2 Pro (Wireless)", true),
        0x00B2 => ("Razer DeathAdder V3", false),
        0x00B6 => ("Razer DeathAdder V3 Pro (Wired)", false),
        0x00B7 => ("Razer DeathAdder V3 Pro (Wireless)", true),
        0x00C2 => ("Razer DeathAdder V3 Pro (Wired, Alt)", false),
        0x00C3 => ("Razer DeathAdder V3 Pro (Wireless, Alt)", true),
        0x00C4 => ("Razer DeathAdder V3 HyperSpeed (Wired)", false),
        0x00C5 => ("Razer DeathAdder V3 HyperSpeed (Wireless)", true),
        0x00BE => ("Razer DeathAdder V4 Pro (Wired)", false),
        0x00BF => ("Razer DeathAdder V4 Pro (Wireless)", true),

        // ── Diamondback family ────────────────────────────────────────────
        0x004C => ("Razer Diamondback Chroma", false),

        // ── HyperPolling Dongle ───────────────────────────────────────────
        0x00B3 => ("Razer HyperPolling Wireless Dongle", false),

        // ── Imperator family ──────────────────────────────────────────────
        0x002F => ("Razer Imperator", false),

        // ── Lancehead family ──────────────────────────────────────────────
        0x0059 => ("Razer Lancehead (Wired)", false),
        0x005A => ("Razer Lancehead (Wireless)", true),
        0x0060 => ("Razer Lancehead Tournament Edition", false),
        0x0070 => ("Razer Lancehead Wireless (Wired)", false),
        0x006F => ("Razer Lancehead Wireless (Receiver)", true),

        // ── Mamba family ──────────────────────────────────────────────────
        0x0024 => ("Razer Mamba 2012 (Wired)", false),
        0x0025 => ("Razer Mamba 2012 (Wireless)", true),
        0x0044 => ("Razer Mamba Chroma (Wired)", false),
        0x0045 => ("Razer Mamba Chroma (Wireless)", true),
        0x0046 => ("Razer Mamba Tournament Edition", false),
        0x006C => ("Razer Mamba Elite", false),
        0x0072 => ("Razer Mamba Wireless (Receiver)", true),
        0x0073 => ("Razer Mamba Wireless (Wired)", false),

        // ── Naga family ───────────────────────────────────────────────────
        0x0015 => ("Razer Naga", false),
        0x001F => ("Razer Naga Epic", false),
        0x002E => ("Razer Naga 2012", false),
        0x0036 => ("Razer Naga Hex (Red)", false),
        0x0040 => ("Razer Naga 2014", false),
        0x0041 => ("Razer Naga Hex", false),
        0x003E => ("Razer Naga Epic Chroma (Wired)", false),
        0x003F => ("Razer Naga Epic Chroma (Wireless)", true),
        0x0050 => ("Razer Naga Hex V2", false),
        0x0053 => ("Razer Naga Chroma", false),
        0x0067 => ("Razer Naga Trinity", false),
        0x008D => ("Razer Naga Left-Handed 2020", false),
        0x008F => ("Razer Naga Pro (Wired)", false),
        0x0090 => ("Razer Naga Pro (Wireless)", true),
        0x0096 => ("Razer Naga X", false),
        0x00A7 => ("Razer Naga V2 Pro (Wired)", false),
        0x00A8 => ("Razer Naga V2 Pro (Wireless)", true),
        0x00B4 => ("Razer Naga V2 HyperSpeed (Receiver)", true),

        // ── Orochi family ─────────────────────────────────────────────────
        0x0013 => ("Razer Orochi 2011", false),
        0x0039 => ("Razer Orochi 2013", false),
        0x0048 => ("Razer Orochi (Wired)", false),
        0x0094 => ("Razer Orochi V2 (Receiver)", true),
        0x0095 => ("Razer Orochi V2 (Bluetooth)", true),

        // ── Ouroboros family ──────────────────────────────────────────────
        0x0032 => ("Razer Ouroboros", false),

        // ── Pro Click family ──────────────────────────────────────────────
        0x0077 => ("Razer Pro Click (Receiver)", true),
        0x0080 => ("Razer Pro Click (Wired)", false),
        0x009A => ("Razer Pro Click Mini (Receiver)", true),
        0x00C7 => ("Razer Pro Click V2 Vertical (Wired)", false),
        0x00C8 => ("Razer Pro Click V2 Vertical (Wireless)", true),
        0x00D0 => ("Razer Pro Click V2 (Wired)", false),
        0x00D1 => ("Razer Pro Click V2 (Wireless)", true),

        // ── Taipan family ─────────────────────────────────────────────────
        0x0034 => ("Razer Taipan", false),

        // ── Viper family ──────────────────────────────────────────────────
        0x0078 => ("Razer Viper", false),
        0x0091 => ("Razer Viper 8KHz", false),
        0x008A => ("Razer Viper Mini", false),
        0x009E => ("Razer Viper Mini SE (Wired)", false),
        0x009F => ("Razer Viper Mini SE (Wireless)", true),
        0x007A => ("Razer Viper Ultimate (Wired)", false),
        0x007B => ("Razer Viper Ultimate (Wireless)", true),
        0x00A5 => ("Razer Viper V2 Pro (Wired)", false),
        0x00A6 => ("Razer Viper V2 Pro (Wireless)", true),
        0x00B8 => ("Razer Viper V3 HyperSpeed", true),
        0x00C0 => ("Razer Viper V3 Pro (Wired)", false),
        0x00C1 => ("Razer Viper V3 Pro (Wireless)", true),

        _ => return None,
    };

    let mut capabilities = vec![DeviceCapability::Lighting(LightingEffect::Static([
        0, 0, 0,
    ]))];
    if has_battery {
        capabilities.push(DeviceCapability::BatteryReporting);
    }

    Some(DeviceProfile {
        name: name.to_string(),
        product_id,
        device_type: DeviceType::Mouse,
        capabilities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_resolves_cobra_pro() {
        let profile = get_device_profile(0x00B0).expect("Cobra Pro Wireless must be in registry");
        assert_eq!(profile.name, "Razer Cobra Pro (Wireless)");
        assert_eq!(profile.device_type, DeviceType::Mouse);
        assert_eq!(profile.product_id, 0x00B0);
        assert!(
            profile
                .capabilities
                .contains(&DeviceCapability::BatteryReporting),
            "Cobra Pro Wireless must advertise BatteryReporting"
        );
    }

    #[test]
    fn test_registry_resolves_deathadder_v2_pro_wired() {
        let profile =
            get_device_profile(0x007C).expect("DeathAdder V2 Pro Wired must be in registry");
        assert_eq!(profile.name, "Razer DeathAdder V2 Pro (Wired)");
        assert_eq!(profile.device_type, DeviceType::Mouse);
        assert!(
            !profile
                .capabilities
                .contains(&DeviceCapability::BatteryReporting),
            "Wired device must NOT advertise BatteryReporting"
        );
    }

    #[test]
    fn test_registry_returns_none_for_unknown_pid() {
        assert!(get_device_profile(0xFFFF).is_none());
    }
}
