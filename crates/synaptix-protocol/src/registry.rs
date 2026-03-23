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
    /// Device supports DPI (sensor resolution) configuration via USB.
    DpiControl,
    /// Device supports sidetone volume control (headsets).
    Sidetone,
    /// Device has a microphone that supports mute toggle.
    Microphone,
    /// Device supports haptic feedback enable/intensity (headsets).
    HapticFeedback,
    /// Device supports THX Spatial Audio toggle.
    ThxSpatialAudio,
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
    /// USB interface number to claim for proprietary HID control transfers.
    ///
    /// Most Razer devices expose their control endpoint on interface `0`.
    /// Composite devices (e.g. the Kraken V4 Pro Hub `0x0568`) route
    /// proprietary commands through a higher-numbered interface (e.g. `3`).
    pub control_interface: u8,
}

/// Looks up a [`DeviceProfile`] by USB product ID (PID).
///
/// Returns `None` when the PID is not in Synaptix's registry.
/// Data sourced from `_reference_openrazer/daemon/openrazer_daemon/hardware/`.
pub fn get_device_profile(product_id: u16) -> Option<DeviceProfile> {
    // (name, device_type, has_battery_reporting, has_dpi_control)
    let (name, device_type, has_battery, has_dpi): (&str, DeviceType, bool, bool) = match product_id
    {
        // ── Abyssus family ────────────────────────────────────────────────
        0x0042 => ("Razer Abyssus", DeviceType::Mouse, false, false),
        0x0020 => ("Razer Abyssus 1800", DeviceType::Mouse, false, false),
        0x005E => ("Razer Abyssus 2000", DeviceType::Mouse, false, false),
        0x006B => ("Razer Abyssus Essential", DeviceType::Mouse, false, false),
        0x005B => ("Razer Abyssus V2", DeviceType::Mouse, false, false),
        0x006A => (
            "Razer Abyssus Elite D.Va Edition",
            DeviceType::Mouse,
            false,
            false,
        ),

        // ── Atheris family ────────────────────────────────────────────────
        0x0062 => ("Razer Atheris (Receiver)", DeviceType::Mouse, true, false),

        // ── Basilisk family ───────────────────────────────────────────────
        0x0064 => ("Razer Basilisk", DeviceType::Mouse, false, false),
        0x0065 => ("Razer Basilisk Essential", DeviceType::Mouse, false, false),
        0x0083 => (
            "Razer Basilisk X HyperSpeed",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0085 => ("Razer Basilisk V2", DeviceType::Mouse, false, false),
        0x0086 => (
            "Razer Basilisk Ultimate (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0088 => (
            "Razer Basilisk Ultimate (Receiver)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0099 => ("Razer Basilisk V3", DeviceType::Mouse, false, false),
        0x00AA => (
            "Razer Basilisk V3 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00AB => (
            "Razer Basilisk V3 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00B9 => (
            "Razer Basilisk V3 X HyperSpeed",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00CB => ("Razer Basilisk V3 35K", DeviceType::Mouse, false, false),
        0x00CC => (
            "Razer Basilisk V3 Pro 35K (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00CD => (
            "Razer Basilisk V3 Pro 35K (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00D6 => (
            "Razer Basilisk V3 Pro 35K Phantom Green (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00D7 => (
            "Razer Basilisk V3 Pro 35K Phantom Green (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Cobra family ──────────────────────────────────────────────────
        0x00A3 => ("Razer Cobra", DeviceType::Mouse, false, false),
        0x00AF => ("Razer Cobra Pro (Wired)", DeviceType::Mouse, false, true),
        0x00B0 => ("Razer Cobra Pro (Wireless)", DeviceType::Mouse, true, true),

        // ── DeathAdder family ─────────────────────────────────────────────
        0x0016 => ("Razer DeathAdder 3.5G", DeviceType::Mouse, false, false),
        0x0029 => (
            "Razer DeathAdder 3.5G (Black)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0037 => ("Razer DeathAdder 2013", DeviceType::Mouse, false, false),
        0x0038 => ("Razer DeathAdder 1800", DeviceType::Mouse, false, false),
        0x0043 => ("Razer DeathAdder Chroma", DeviceType::Mouse, false, false),
        0x004F => ("Razer DeathAdder 2000", DeviceType::Mouse, false, false),
        0x0054 => ("Razer DeathAdder 3500", DeviceType::Mouse, false, false),
        0x005C => ("Razer DeathAdder Elite", DeviceType::Mouse, false, false),
        0x006E => (
            "Razer DeathAdder Essential",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0071 => (
            "Razer DeathAdder Essential (White Edition)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0084 => ("Razer DeathAdder V2", DeviceType::Mouse, false, false),
        0x008C => ("Razer DeathAdder V2 Mini", DeviceType::Mouse, false, false),
        0x0098 => (
            "Razer DeathAdder Essential (2021)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x009C => (
            "Razer DeathAdder V2 X HyperSpeed",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00A1 => ("Razer DeathAdder V2 Lite", DeviceType::Mouse, false, false),
        0x007C => (
            "Razer DeathAdder V2 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x007D => (
            "Razer DeathAdder V2 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00B2 => ("Razer DeathAdder V3", DeviceType::Mouse, false, false),
        0x00B6 => (
            "Razer DeathAdder V3 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00B7 => (
            "Razer DeathAdder V3 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00C2 => (
            "Razer DeathAdder V3 Pro (Wired, Alt)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00C3 => (
            "Razer DeathAdder V3 Pro (Wireless, Alt)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00C4 => (
            "Razer DeathAdder V3 HyperSpeed (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00C5 => (
            "Razer DeathAdder V3 HyperSpeed (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00BE => (
            "Razer DeathAdder V4 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00BF => (
            "Razer DeathAdder V4 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Diamondback family ────────────────────────────────────────────
        0x004C => ("Razer Diamondback Chroma", DeviceType::Mouse, false, false),

        // ── HyperPolling Dongle ───────────────────────────────────────────
        0x00B3 => (
            "Razer HyperPolling Wireless Dongle",
            DeviceType::Mouse,
            false,
            false,
        ),

        // ── Imperator family ──────────────────────────────────────────────
        0x002F => ("Razer Imperator", DeviceType::Mouse, false, false),

        // ── Lancehead family ──────────────────────────────────────────────
        0x0059 => ("Razer Lancehead (Wired)", DeviceType::Mouse, false, false),
        0x005A => ("Razer Lancehead (Wireless)", DeviceType::Mouse, true, false),
        0x0060 => (
            "Razer Lancehead Tournament Edition",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0070 => (
            "Razer Lancehead Wireless (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x006F => (
            "Razer Lancehead Wireless (Receiver)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Mamba family ──────────────────────────────────────────────────
        0x0024 => ("Razer Mamba 2012 (Wired)", DeviceType::Mouse, false, false),
        0x0025 => (
            "Razer Mamba 2012 (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0044 => (
            "Razer Mamba Chroma (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x0045 => (
            "Razer Mamba Chroma (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0046 => (
            "Razer Mamba Tournament Edition",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x006C => ("Razer Mamba Elite", DeviceType::Mouse, false, false),
        0x0072 => (
            "Razer Mamba Wireless (Receiver)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0073 => (
            "Razer Mamba Wireless (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),

        // ── Naga family ───────────────────────────────────────────────────
        0x0015 => ("Razer Naga", DeviceType::Mouse, false, false),
        0x001F => ("Razer Naga Epic", DeviceType::Mouse, false, false),
        0x002E => ("Razer Naga 2012", DeviceType::Mouse, false, false),
        0x0036 => ("Razer Naga Hex (Red)", DeviceType::Mouse, false, false),
        0x0040 => ("Razer Naga 2014", DeviceType::Mouse, false, false),
        0x0041 => ("Razer Naga Hex", DeviceType::Mouse, false, false),
        0x003E => (
            "Razer Naga Epic Chroma (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x003F => (
            "Razer Naga Epic Chroma (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x0050 => ("Razer Naga Hex V2", DeviceType::Mouse, false, false),
        0x0053 => ("Razer Naga Chroma", DeviceType::Mouse, false, false),
        0x0067 => ("Razer Naga Trinity", DeviceType::Mouse, false, false),
        0x008D => (
            "Razer Naga Left-Handed 2020",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x008F => ("Razer Naga Pro (Wired)", DeviceType::Mouse, false, false),
        0x0090 => ("Razer Naga Pro (Wireless)", DeviceType::Mouse, true, false),
        0x0096 => ("Razer Naga X", DeviceType::Mouse, false, false),
        0x00A7 => ("Razer Naga V2 Pro (Wired)", DeviceType::Mouse, false, false),
        0x00A8 => (
            "Razer Naga V2 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00B4 => (
            "Razer Naga V2 HyperSpeed (Receiver)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Orochi family ─────────────────────────────────────────────────
        0x0013 => ("Razer Orochi 2011", DeviceType::Mouse, false, false),
        0x0039 => ("Razer Orochi 2013", DeviceType::Mouse, false, false),
        0x0048 => ("Razer Orochi (Wired)", DeviceType::Mouse, false, false),
        0x0094 => ("Razer Orochi V2 (Receiver)", DeviceType::Mouse, true, false),
        0x0095 => (
            "Razer Orochi V2 (Bluetooth)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Ouroboros family ──────────────────────────────────────────────
        0x0032 => ("Razer Ouroboros", DeviceType::Mouse, false, false),

        // ── Pro Click family ──────────────────────────────────────────────
        0x0077 => ("Razer Pro Click (Receiver)", DeviceType::Mouse, true, false),
        0x0080 => ("Razer Pro Click (Wired)", DeviceType::Mouse, false, false),
        0x009A => (
            "Razer Pro Click Mini (Receiver)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00C7 => (
            "Razer Pro Click V2 Vertical (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00C8 => (
            "Razer Pro Click V2 Vertical (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00D0 => (
            "Razer Pro Click V2 (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00D1 => (
            "Razer Pro Click V2 (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ── Taipan family ─────────────────────────────────────────────────
        0x0034 => ("Razer Taipan", DeviceType::Mouse, false, false),

        // ── Viper family ──────────────────────────────────────────────────
        0x0078 => ("Razer Viper", DeviceType::Mouse, false, false),
        0x0091 => ("Razer Viper 8KHz", DeviceType::Mouse, false, false),
        0x008A => ("Razer Viper Mini", DeviceType::Mouse, false, false),
        0x009E => (
            "Razer Viper Mini SE (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x009F => (
            "Razer Viper Mini SE (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x007A => (
            "Razer Viper Ultimate (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x007B => (
            "Razer Viper Ultimate (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00A5 => (
            "Razer Viper V2 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00A6 => (
            "Razer Viper V2 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),
        0x00B8 => ("Razer Viper V3 HyperSpeed", DeviceType::Mouse, true, false),
        0x00C0 => (
            "Razer Viper V3 Pro (Wired)",
            DeviceType::Mouse,
            false,
            false,
        ),
        0x00C1 => (
            "Razer Viper V3 Pro (Wireless)",
            DeviceType::Mouse,
            true,
            false,
        ),

        // ════════════════════════════════════════════════════════════════
        // KEYBOARDS — sourced from openrazer/hardware/keyboards.py
        // ════════════════════════════════════════════════════════════════

        // ── Anansi ────────────────────────────────────────────────────
        0x010F => ("Razer Anansi", DeviceType::Keyboard, false, false),

        // ── BlackWidow family ─────────────────────────────────────────
        0x010D => (
            "Razer BlackWidow Ultimate 2012",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x011A => (
            "Razer BlackWidow Ultimate 2013",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x011B => (
            "Razer BlackWidow Stealth",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x010E => (
            "Razer BlackWidow Stealth Edition",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x011C => (
            "Razer BlackWidow Tournament Edition 2014",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0203 => (
            "Razer BlackWidow Chroma",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0209 => (
            "Razer BlackWidow Chroma Tournament Edition",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0211 => (
            "Razer BlackWidow Chroma (Overwatch)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0214 => (
            "Razer BlackWidow Ultimate 2016",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0216 => (
            "Razer BlackWidow X Chroma",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0217 => (
            "Razer BlackWidow X Ultimate",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x021A => (
            "Razer BlackWidow X Tournament Edition Chroma",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0221 => (
            "Razer BlackWidow Chroma V2",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0228 => ("Razer BlackWidow Elite", DeviceType::Keyboard, false, false),
        0x0235 => ("Razer BlackWidow Lite", DeviceType::Keyboard, false, false),
        0x0237 => (
            "Razer BlackWidow Essential",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0241 => ("Razer BlackWidow 2019", DeviceType::Keyboard, false, false),
        0x024E => ("Razer BlackWidow V3", DeviceType::Keyboard, false, false),
        0x025A => (
            "Razer BlackWidow V3 Pro (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x025C => (
            "Razer BlackWidow V3 Pro (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),
        0x0258 => (
            "Razer BlackWidow V3 Mini HyperSpeed (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0271 => (
            "Razer BlackWidow V3 Mini HyperSpeed (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),
        0x0A24 => (
            "Razer BlackWidow V3 TKL",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0287 => ("Razer BlackWidow V4", DeviceType::Keyboard, false, false),
        0x028D => (
            "Razer BlackWidow V4 Pro",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02A5 => (
            "Razer BlackWidow V4 75%",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0293 => ("Razer BlackWidow V4 X", DeviceType::Keyboard, false, false),
        0x02B9 => (
            "Razer BlackWidow V4 Mini HyperSpeed (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02BA => (
            "Razer BlackWidow V4 Mini HyperSpeed (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),
        0x02D7 => (
            "Razer BlackWidow V4 TKL HyperSpeed (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02D5 => (
            "Razer BlackWidow V4 TKL HyperSpeed (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),

        // ── Cynosa family ─────────────────────────────────────────────
        0x022A => ("Razer Cynosa Chroma", DeviceType::Keyboard, false, false),
        0x022C => (
            "Razer Cynosa Chroma Pro",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x023F => ("Razer Cynosa Lite", DeviceType::Keyboard, false, false),
        0x025E => ("Razer Cynosa V2", DeviceType::Keyboard, false, false),

        // ── DeathStalker family ───────────────────────────────────────
        0x0202 => (
            "Razer DeathStalker Expert",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0118 => (
            "Razer DeathStalker Essential",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0204 => (
            "Razer DeathStalker Chroma",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0295 => ("Razer DeathStalker V2", DeviceType::Keyboard, false, false),
        0x0292 => (
            "Razer DeathStalker V2 Pro (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0290 => (
            "Razer DeathStalker V2 Pro (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),
        0x0298 => (
            "Razer DeathStalker V2 Pro TKL (Wired)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0296 => (
            "Razer DeathStalker V2 Pro TKL (Wireless)",
            DeviceType::Keyboard,
            true,
            false,
        ),

        // ── Huntsman family ───────────────────────────────────────────
        0x0226 => ("Razer Huntsman Elite", DeviceType::Keyboard, false, false),
        0x0227 => ("Razer Huntsman", DeviceType::Keyboard, false, false),
        0x0243 => (
            "Razer Huntsman Tournament Edition",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0257 => ("Razer Huntsman Mini", DeviceType::Keyboard, false, false),
        0x0269 => ("Razer Huntsman Mini JP", DeviceType::Keyboard, false, false),
        0x0282 => (
            "Razer Huntsman Mini Analog",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026B => (
            "Razer Huntsman V2 Tenkeyless",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026C => ("Razer Huntsman V2", DeviceType::Keyboard, false, false),
        0x0266 => (
            "Razer Huntsman V2 Analog",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02A6 => ("Razer Huntsman V3 Pro", DeviceType::Keyboard, false, false),
        0x02A7 => (
            "Razer Huntsman V3 Pro TKL",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02B0 => (
            "Razer Huntsman V3 Pro Mini",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02CF => (
            "Razer Huntsman V3 Pro 8KHz",
            DeviceType::Keyboard,
            false,
            false,
        ),

        // ── Nostromo / Tartarus / Orbweaver (macro pads) ──────────────
        0x0111 => ("Razer Nostromo", DeviceType::Keyboard, false, false),
        0x0201 => ("Razer Tartarus", DeviceType::Keyboard, false, false),
        0x0208 => ("Razer Tartarus Chroma", DeviceType::Keyboard, false, false),
        0x022B => ("Razer Tartarus V2", DeviceType::Keyboard, false, false),
        0x0244 => ("Razer Tartarus Pro", DeviceType::Keyboard, false, false),
        0x0113 => ("Razer Orbweaver", DeviceType::Keyboard, false, false),
        0x0207 => ("Razer Orbweaver Chroma", DeviceType::Keyboard, false, false),

        // ── Ornata family ─────────────────────────────────────────────
        0x021F => ("Razer Ornata", DeviceType::Keyboard, false, false),
        0x021E => ("Razer Ornata Chroma", DeviceType::Keyboard, false, false),
        0x025D => ("Razer Ornata V2", DeviceType::Keyboard, false, false),
        0x02A1 => ("Razer Ornata V3", DeviceType::Keyboard, false, false),
        0x028F => ("Razer Ornata V3 (Alt)", DeviceType::Keyboard, false, false),
        0x0294 => ("Razer Ornata V3 X", DeviceType::Keyboard, false, false),
        0x02A2 => (
            "Razer Ornata V3 X (Alt)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x02A3 => (
            "Razer Ornata V3 Tenkeyless",
            DeviceType::Keyboard,
            false,
            false,
        ),

        // ── Razer Blade laptop keyboards ──────────────────────────────
        0x0205 => ("Razer Blade Stealth", DeviceType::Keyboard, false, false),
        0x0220 => (
            "Razer Blade Stealth (Late 2016)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x022D => (
            "Razer Blade Stealth (Mid 2017)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0232 => (
            "Razer Blade Stealth (Late 2017)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0239 => (
            "Razer Blade Stealth (2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x024A => (
            "Razer Blade Stealth (Late 2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0252 => (
            "Razer Blade Stealth (Early 2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0259 => (
            "Razer Blade Stealth (Late 2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x020F => ("Razer Blade QHD", DeviceType::Keyboard, false, false),
        0x0224 => (
            "Razer Blade (Late 2016)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0210 => (
            "Razer Blade Pro (Late 2016)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0225 => ("Razer Blade Pro (2017)", DeviceType::Keyboard, false, false),
        0x022F => (
            "Razer Blade Pro (2017 Full HD)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0233 => ("Razer Blade 15 (2018)", DeviceType::Keyboard, false, false),
        0x0240 => (
            "Razer Blade 15 Mercury (2018)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x023B => (
            "Razer Blade 15 Base (2018)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x023A => (
            "Razer Blade 15 Advanced (2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0245 => (
            "Razer Blade 15 Mercury (Mid 2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0246 => (
            "Razer Blade 15 Base (2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0253 => (
            "Razer Blade 15 Advanced (2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0255 => (
            "Razer Blade 15 Base (Early 2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0268 => (
            "Razer Blade 15 Base (Late 2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0276 => (
            "Razer Blade 15 Advanced (2021)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026D => (
            "Razer Blade 15 Advanced (Early 2021)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026F => (
            "Razer Blade 15 Base (Early 2021)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x027A => (
            "Razer Blade 15 Base (Early 2022)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x028A => (
            "Razer Blade 15 Advanced (Early 2022)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x029E => ("Razer Blade 15 (2023)", DeviceType::Keyboard, false, false),
        0x0234 => ("Razer Blade Pro (2019)", DeviceType::Keyboard, false, false),
        0x024C => (
            "Razer Blade Pro (Late 2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x024B => (
            "Razer Blade Advanced (Late 2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0256 => (
            "Razer Blade Pro (Early 2020)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x0279 => (
            "Razer Blade 17 Pro (2021)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026E => (
            "Razer Blade 17 Pro (Early 2021)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x028B => ("Razer Blade 17 (2022)", DeviceType::Keyboard, false, false),
        0x02A0 => ("Razer Blade 18 (2023)", DeviceType::Keyboard, false, false),
        0x02B8 => ("Razer Blade 18 (2024)", DeviceType::Keyboard, false, false),
        0x02C7 => ("Razer Blade 18 (2025)", DeviceType::Keyboard, false, false),
        0x0270 => ("Razer Blade 14 (2021)", DeviceType::Keyboard, false, false),
        0x028C => ("Razer Blade 14 (2022)", DeviceType::Keyboard, false, false),
        0x029D => ("Razer Blade 14 (2023)", DeviceType::Keyboard, false, false),
        0x02B6 => ("Razer Blade 14 (2024)", DeviceType::Keyboard, false, false),
        0x02C5 => ("Razer Blade 14 (2025)", DeviceType::Keyboard, false, false),
        0x029F => ("Razer Blade 16 (2023)", DeviceType::Keyboard, false, false),
        0x02C6 => ("Razer Blade 16 (2025)", DeviceType::Keyboard, false, false),
        0x024D => (
            "Razer Blade 15 Studio Edition (2019)",
            DeviceType::Keyboard,
            false,
            false,
        ),
        0x026A => ("Razer Book (2020)", DeviceType::Keyboard, false, false),

        // ── Audio / Headsets ──────────────────────────────────────────────
        0x0501 => ("Razer Kraken 7.1", DeviceType::Audio, false, false),
        0x0504 => ("Razer Kraken 7.1 Chroma", DeviceType::Audio, false, false),
        0x0506 => (
            "Razer Kraken 7.1 (Alternate)",
            DeviceType::Audio,
            false,
            false,
        ),
        0x0510 => ("Razer Kraken 7.1 V2", DeviceType::Audio, false, false),
        0x0520 => (
            "Razer Kraken Tournament Edition",
            DeviceType::Audio,
            false,
            false,
        ),
        0x0527 => ("Razer Kraken Ultimate", DeviceType::Audio, false, false),
        0x0560 => ("Razer Kraken Kitty V2", DeviceType::Audio, false, false),
        0x0567 => (
            "Razer Kraken V4 Pro (Receiver)",
            DeviceType::Audio,
            false,
            false,
        ),
        0x0568 => ("Razer Kraken V4 Pro", DeviceType::Audio, false, false),
        0x056c => (
            "Razer Kraken V4 Pro (Main)",
            DeviceType::Audio,
            false,
            false,
        ),
        0x0F19 => (
            "Razer Kraken Kitty Edition",
            DeviceType::Audio,
            false,
            false,
        ),

        _ => return None,
    };

    let mut capabilities = vec![DeviceCapability::Lighting(LightingEffect::Static([
        0, 0, 0,
    ]))];
    if has_battery {
        capabilities.push(DeviceCapability::BatteryReporting);
    }
    if has_dpi {
        capabilities.push(DeviceCapability::DpiControl);
    }
    // Headset capabilities — Sidetone + Microphone for all Kraken audio devices.
    if matches!(device_type, DeviceType::Audio) {
        capabilities.push(DeviceCapability::Sidetone);
        capabilities.push(DeviceCapability::Microphone);
    }
    // Kraken V4 Pro (both the headset 0x0568 and its USB receiver/hub 0x0567)
    // supports haptics and THX Spatial Audio.
    if matches!(product_id, 0x0567 | 0x0568 | 0x056c) {
        capabilities.push(DeviceCapability::HapticFeedback);
        capabilities.push(DeviceCapability::ThxSpatialAudio);
    }

    // The Kraken V4 Pro Hub (0x0568) and main device (0x056c) use a composite
    // USB configuration; proprietary HID commands must be routed to Interface 4.
    let control_interface: u8 = if matches!(product_id, 0x0568 | 0x056c) {
        4
    } else {
        0
    };

    Some(DeviceProfile {
        name: name.to_string(),
        product_id,
        device_type,
        capabilities,
        control_interface,
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

    #[test]
    fn test_registry_resolves_blackwidow_v3() {
        let profile = get_device_profile(0x024E).expect("BlackWidow V3 must be in registry");
        assert_eq!(profile.name, "Razer BlackWidow V3");
        assert_eq!(profile.device_type, DeviceType::Keyboard);
        assert_eq!(profile.product_id, 0x024E);
        assert!(
            !profile
                .capabilities
                .contains(&DeviceCapability::BatteryReporting),
            "Wired keyboard must NOT advertise BatteryReporting"
        );
    }

    #[test]
    fn test_registry_resolves_blackwidow_v3_pro_wireless() {
        let profile =
            get_device_profile(0x025C).expect("BlackWidow V3 Pro Wireless must be in registry");
        assert_eq!(profile.name, "Razer BlackWidow V3 Pro (Wireless)");
        assert_eq!(profile.device_type, DeviceType::Keyboard);
        assert!(
            profile
                .capabilities
                .contains(&DeviceCapability::BatteryReporting),
            "Wireless keyboard must advertise BatteryReporting"
        );
    }

    #[test]
    fn test_registry_resolves_kraken_kitty_edition() {
        let profile = get_device_profile(0x0F19).expect("Kraken Kitty Edition must be in registry");
        assert_eq!(profile.name, "Razer Kraken Kitty Edition");
        assert_eq!(profile.device_type, DeviceType::Audio);
        assert_eq!(profile.product_id, 0x0F19);
    }

    #[test]
    fn test_registry_resolves_kraken_v4_pro() {
        let profile = get_device_profile(0x0568).expect("Kraken V4 Pro must be in registry");
        assert_eq!(profile.name, "Razer Kraken V4 Pro");
        assert_eq!(profile.device_type, DeviceType::Audio);
        // Hub requires interface 3 for proprietary HID commands.
        // Wireshark confirmed: wIndex = 0x0004 (interface 4) for haptic payloads.
        assert_eq!(profile.control_interface, 4);
    }

    #[test]
    fn test_registry_resolves_kraken_v4_pro_main() {
        let profile =
            get_device_profile(0x056c).expect("Kraken V4 Pro (Main) must be in registry");
        assert_eq!(profile.name, "Razer Kraken V4 Pro (Main)");
        assert_eq!(profile.device_type, DeviceType::Audio);
        assert_eq!(profile.control_interface, 4);
        assert!(
            profile
                .capabilities
                .iter()
                .any(|c| matches!(c, DeviceCapability::HapticFeedback)),
            "Kraken V4 Pro (Main) must have HapticFeedback capability"
        );
    }

    #[test]
    fn test_control_interface_defaults_to_zero() {
        // Mice and keyboards must use interface 0 (the default).
        let cobra = get_device_profile(0x00B0).expect("Cobra Pro must be in registry");
        assert_eq!(cobra.control_interface, 0);
        let bw = get_device_profile(0x024E).expect("BlackWidow V3 must be in registry");
        assert_eq!(bw.control_interface, 0);
    }
}
