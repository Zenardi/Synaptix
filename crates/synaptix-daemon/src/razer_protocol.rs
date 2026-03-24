use std::sync::atomic::{AtomicU8, Ordering};

/// USB Vendor ID for all Razer devices.
pub const RAZER_VID: u16 = 0x1532;

/// Total length of a Razer USB HID report in bytes.
pub const REPORT_LEN: usize = 90;

// ── Constants (derived from razercommon.h / razerchromacommon.c) ──────────────

/// Command class: Extended Matrix Effects.
pub const CMD_CLASS: u8 = 0x0F;
/// Command ID: Set Effect.
pub const CMD_ID: u8 = 0x02;
/// Volatile variable storage (changes are lost on power-cycle).
pub const VARSTORE: u8 = 0x01;
/// Static solid-colour effect ID.
pub const EFFECT_STATIC: u8 = 0x01;
/// Single-colour breathing effect ID.
pub const EFFECT_BREATHING_SINGLE: u8 = 0x02;
/// Auto spectrum-cycling effect ID.
pub const EFFECT_SPECTRUM: u8 = 0x04;

// ── Per-device transaction IDs ────────────────────────────────────────────────

/// Transaction ID for DeathAdder V2 Pro and similar older wireless mice.
pub const TRANSACTION_ID_DA: u8 = 0x3F;

/// Transaction ID for Cobra Pro, Basilisk V3 Pro, and newer wireless mice.
pub const TRANSACTION_ID_COBRA: u8 = 0x1F;

// ── Per-device LED zone IDs ───────────────────────────────────────────────────

/// Zero / catch-all LED zone (used by Cobra Pro and many newer mice).
pub const LED_ZERO: u8 = 0x00;

// ── USB response wait times (from razermouse_driver.h) ────────────────────────

/// Minimum sleep (µs) after SET_REPORT before issuing GET_REPORT for standard mice.
/// Source: `RAZER_MOUSE_WAIT_MIN_US` in razermouse_driver.h
pub const _WAIT_STANDARD_US: u64 = 600;

/// Minimum sleep (µs) for Cobra Pro, Basilisk V3 Pro, DA V3 Pro, and other new receivers.
/// Source: `RAZER_NEW_MOUSE_RECEIVER_WAIT_MIN_US` in razermouse_driver.h
pub const WAIT_NEW_RECEIVER_US: u64 = 31_000;

/// Minimum sleep (µs) for Viper Ultimate, DA V2 Pro, Viper V3 Pro, and Viper receivers.
/// Source: `RAZER_VIPER_MOUSE_RECEIVER_WAIT_MIN_US` in razermouse_driver.h
pub const _WAIT_VIPER_RECEIVER_US: u64 = 59_900;

/// Backlight LED zone — covers the logo on DA V2 Pro and similar mice.
pub const LED_BACKLIGHT: u8 = 0x05;

// ─────────────────────────────────────────────────────────────────────────────

/// Calculates and inserts the Razer HID report checksum in place.
///
/// The checksum is the XOR of bytes `[2..88]` stored at index `88`.
/// Byte `89` is always `0x00` (reserved). Protocol source: `razercommon.c`.
fn calculate_razer_checksum(payload: &mut [u8; REPORT_LEN]) {
    payload[88] = payload[2..88].iter().fold(0u8, |acc, &byte| acc ^ byte);
}

/// Builds a 90-byte Razer HID report for the Extended Matrix Static effect.
///
/// `transaction_id` and `led_id` are device-specific — use the `TRANSACTION_ID_*`
/// and `LED_*` constants above.
///
/// Layout (`razercommon.h` → `struct razer_report`):
/// ```text
/// Byte  0     status            = 0x00  (new command)
/// Byte  1     transaction_id    (device-specific)
/// Bytes 2-3   remaining_packets = 0x0000
/// Byte  4     protocol_type     = 0x00
/// Byte  5     data_size         = 0x09  (9 argument bytes)
/// Byte  6     command_class     = 0x0F
/// Byte  7     command_id        = 0x02
/// Byte  8     args[0]           = 0x01  VARSTORE
/// Byte  9     args[1]           = led_id
/// Byte 10     args[2]           = 0x01  EFFECT_STATIC
/// Bytes 11-12 args[3-4]         = 0x00  (padding)
/// Byte 13     args[5]           = 0x01  (colour count)
/// Byte 14     args[6]           = r
/// Byte 15     args[7]           = g
/// Byte 16     args[8]           = b
/// Bytes 17-87 args[9-79]        = 0x00  (padding)
/// Byte 88     crc               = XOR(bytes[2..88])
/// Byte 89     reserved          = 0x00
/// ```
pub fn build_static_color_payload(
    transaction_id: u8,
    led_id: u8,
    r: u8,
    g: u8,
    b: u8,
) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];

    buf[1] = transaction_id;
    buf[5] = 0x09; // data_size
    buf[6] = CMD_CLASS;
    buf[7] = CMD_ID;
    buf[8] = VARSTORE;
    buf[9] = led_id;
    buf[10] = EFFECT_STATIC;
    buf[13] = 0x01; // colour count
    buf[14] = r;
    buf[15] = g;
    buf[16] = b;

    calculate_razer_checksum(&mut buf);

    buf
}

/// Extracts the battery percentage from a 90-byte Razer HID GET_REPORT response.
///
/// `response[9]` (`arguments[1]`) holds a raw 0–255 level from the firmware.
/// This maps linearly to 0–100 % using integer arithmetic.
///
/// Used by the TDD test suite.
#[cfg(test)]
pub fn parse_battery_response(response: &[u8; REPORT_LEN]) -> u8 {
    let raw = response[9];
    ((raw as u16 * 100) / 255) as u8
}

// ── Sensor / DPI control ──────────────────────────────────────────────────────

/// Command class for sensor / DPI configuration.
/// Source: `razer_chroma_misc_set_dpi_xy` in `razerchromacommon.c`.
pub const CMD_CLASS_SENSOR: u8 = 0x04;

/// Command ID: set X/Y DPI independently.
pub const CMD_ID_SET_DPI: u8 = 0x05;

/// Builds a 90-byte Razer HID report that sets the mouse DPI for both axes.
///
/// DPI values are encoded as Big-Endian 16-bit pairs in the `arguments[]`
/// section of the report (source: `razer_chroma_misc_set_dpi_xy`):
///
/// ```text
/// Byte  1     transaction_id            (device-specific)
/// Byte  5     data_size      = 0x07     (7 argument bytes)
/// Byte  6     command_class  = 0x04
/// Byte  7     command_id     = 0x05
/// Byte  8     args[0]        = VARSTORE (0x01)
/// Byte  9     args[1]        = dpi_x >> 8          (high byte)
/// Byte 10     args[2]        = dpi_x & 0xFF         (low byte)
/// Byte 11     args[3]        = dpi_y >> 8          (high byte)
/// Byte 12     args[4]        = dpi_y & 0xFF         (low byte)
/// Byte 88     crc            = XOR(bytes[2..88])
/// ```
///
/// Example — 800 DPI: `800 = 0x0320`  → high = `0x03`, low = `0x20`.
pub fn build_set_dpi_payload(transaction_id: u8, dpi_x: u16, dpi_y: u16) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = transaction_id;
    buf[5] = 0x07; // data_size
    buf[6] = CMD_CLASS_SENSOR;
    buf[7] = CMD_ID_SET_DPI;
    buf[8] = VARSTORE;
    buf[9] = (dpi_x >> 8) as u8;
    buf[10] = (dpi_x & 0xFF) as u8;
    buf[11] = (dpi_y >> 8) as u8;
    buf[12] = (dpi_y & 0xFF) as u8;
    calculate_razer_checksum(&mut buf);
    buf
}

// ── Response status bytes (razercommon.h) ─────────────────────────────────────

/// The firmware is still processing a previous command — wait and retry.
pub const STATUS_BUSY: u8 = 0x01;
/// Command was accepted and the response arguments are valid.
pub const STATUS_SUCCESSFUL: u8 = 0x02;
/// Firmware reported a command failure.
pub const STATUS_FAILURE: u8 = 0x03;
/// Command timed out in firmware.
pub const STATUS_TIMEOUT: u8 = 0x04;
/// Command is not supported by this device/firmware.
pub const STATUS_NOT_SUPPORTED: u8 = 0x05;

/// Validates a GET_REPORT response against the originating request bytes.
///
/// Mirrors the checks in `razer_send_payload` (razermouse_driver.c):
/// - `response[0]` must be `STATUS_SUCCESSFUL` (0x02).  If `STATUS_BUSY`
///   (0x01), returns `Err(false)` to signal "retry".  Any other status is a
///   hard failure — returns `Err(true)`.
/// - `response[6]` must equal the request `command_class`.
/// - `response[7]` must equal the request `command_id`.
///
/// Returns `Ok(())` when the response is valid and arguments can be read.
pub fn validate_response(
    response: &[u8; REPORT_LEN],
    command_class: u8,
    command_id: u8,
) -> Result<(), bool> {
    match response[0] {
        STATUS_SUCCESSFUL => {}
        STATUS_BUSY => return Err(false), // soft error — caller should retry
        STATUS_FAILURE | STATUS_TIMEOUT | STATUS_NOT_SUPPORTED => {
            eprintln!(
                "[USB] Response status=0x{:02x} for cmd 0x{command_class:02x}/0x{command_id:02x}",
                response[0]
            );
            return Err(true);
        }
        _ => {
            eprintln!(
                "[USB] Unknown response status=0x{:02x} for cmd 0x{command_class:02x}/0x{command_id:02x}",
                response[0]
            );
            return Err(true);
        }
    }
    if response[6] != command_class || response[7] != command_id {
        eprintln!(
            "[USB] Response echo mismatch: expected class=0x{command_class:02x} id=0x{command_id:02x}, \
             got class=0x{:02x} id=0x{:02x}",
            response[6], response[7]
        );
        return Err(true);
    }
    Ok(())
}

// ── Battery / Power queries ───────────────────────────────────────────────────

/// Command class for battery and power management queries.
pub const CMD_CLASS_BATTERY: u8 = 0x07;

/// Command ID: get battery level.  High bit set = device-to-host query.
/// Response: `arguments[1]` contains 0-255 raw level.
pub const CMD_ID_BATTERY_LEVEL: u8 = 0x80;

/// Command ID: get charging status.
/// Response: `arguments[1]` is 0 (discharging) or 1 (charging).
pub const CMD_ID_CHARGING_STATUS: u8 = 0x84;

/// Builds the 90-byte HID report that queries the device's battery level.
///
/// Send this with SET_REPORT, sleep ≥1 ms, then read 90 bytes with GET_REPORT.
/// The battery level (0–255) will be in the response at `buf[9]` (arguments[1]).
pub fn build_battery_query_payload(transaction_id: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = transaction_id;
    buf[5] = 0x02; // data_size
    buf[6] = CMD_CLASS_BATTERY;
    buf[7] = CMD_ID_BATTERY_LEVEL;
    calculate_razer_checksum(&mut buf);
    buf
}

/// Builds the 90-byte HID report that queries the device's charging status.
///
/// Send this with SET_REPORT, sleep ≥1 ms, then read 90 bytes with GET_REPORT.
/// The charging flag (0 or 1) will be in the response at `buf[9]` (arguments[1]).
pub fn build_charging_query_payload(transaction_id: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = transaction_id;
    buf[5] = 0x02; // data_size
    buf[6] = CMD_CLASS_BATTERY;
    buf[7] = CMD_ID_CHARGING_STATUS;
    calculate_razer_checksum(&mut buf);
    buf
}

// ── Lighting effect builders ──────────────────────────────────────────────────

/// Builds a 90-byte Razer HID report for the single-colour Breathing effect.
///
/// Same header layout as `build_static_color_payload` but with effect byte
/// `0x02` (`EFFECT_BREATHING_SINGLE`) instead of `0x01`.
///
/// ```text
/// Byte  8  args[0] = VARSTORE (0x01)
/// Byte  9  args[1] = led_id
/// Byte 10  args[2] = 0x02  BREATHING_SINGLE
/// Byte 13  args[5] = 0x01  colour count
/// Bytes 14-16       r, g, b
/// ```
pub fn build_breathing_payload(
    transaction_id: u8,
    led_id: u8,
    r: u8,
    g: u8,
    b: u8,
) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = transaction_id;
    buf[5] = 0x09;
    buf[6] = CMD_CLASS;
    buf[7] = CMD_ID;
    buf[8] = VARSTORE;
    buf[9] = led_id;
    buf[10] = EFFECT_BREATHING_SINGLE;
    buf[13] = 0x01;
    buf[14] = r;
    buf[15] = g;
    buf[16] = b;
    calculate_razer_checksum(&mut buf);
    buf
}

/// Builds a 90-byte Razer HID report for the Spectrum (auto colour-cycle) effect.
///
/// Spectrum carries no colour data; data_size is `0x03` (VARSTORE + led_id + effect).
///
/// ```text
/// Byte  8  args[0] = VARSTORE (0x01)
/// Byte  9  args[1] = led_id
/// Byte 10  args[2] = 0x04  SPECTRUM
/// ```
pub fn build_spectrum_payload(transaction_id: u8, led_id: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = transaction_id;
    buf[5] = 0x03;
    buf[6] = CMD_CLASS;
    buf[7] = CMD_ID;
    buf[8] = VARSTORE;
    buf[9] = led_id;
    buf[10] = EFFECT_SPECTRUM;
    calculate_razer_checksum(&mut buf);
    buf
}

/// Builds a 90-byte Razer HID report for the Kraken V4 Pro static lighting.
///
/// The Kraken V4 Pro uses a simplified header layout (no VARSTORE/LED zone):
/// ```text
/// Byte  1     transaction_id = 0xFF
/// Byte  5     data_size      = 0x05
/// Byte  6     command_class  = 0x0F
/// Byte  7     command_id     = 0x02
/// Bytes 8-10  args[0-2]      = r, g, b
/// Byte 88     crc            = XOR(bytes[2..88])
/// Byte 89     reserved       = 0x00
/// ```
pub fn build_kraken_v4_static_payload(r: u8, g: u8, b: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = 0xFF;
    buf[5] = 0x05;
    buf[6] = 0x0F;
    buf[7] = 0x02;
    buf[8] = r;
    buf[9] = g;
    buf[10] = b;
    calculate_razer_checksum(&mut buf);
    buf
}

// ── Headset audio / haptic commands ──────────────────────────────────────────
//
// These constants are based on the historical Kraken V3 HyperSense protocol
// as documented by the community.  They share the same command class (0x04)
// as the sensor/DPI subsystem.  The specific command IDs (0x04 for sidetone,
// 0x07 for haptics) are **baseline guesses** and should be verified against
// Wireshark captures on Kraken V4 Pro hardware before relying on them.

/// Transaction ID for Kraken headsets (V3/V4 generation).
/// Source: Kraken V3 HyperSense community reverse-engineering.
pub const TRANSACTION_ID_HEADSET: u8 = 0xFF;

/// Command ID: set sidetone volume.
/// Source: Kraken V3 community reverse-engineering.
/// ⚠️  Wireshark verification required for Kraken V4 Pro (PID 0x0568).
pub const CMD_ID_SET_SIDETONE: u8 = 0x04;

/// Command ID: set haptic feedback intensity (HyperSense).
/// Source: Kraken V3 HyperSense community reverse-engineering.
/// ⚠️  Wireshark verification required for Kraken V4 Pro (PID 0x0568).
pub const CMD_ID_SET_HAPTIC_INTENSITY: u8 = 0x07;

/// Builds a 90-byte Razer HID report that sets the headset sidetone volume.
///
/// Sidetone lets the wearer hear their own voice through the ear cups.
/// Level range: 0 (silent) – 100 (full).
///
/// ```text
/// Byte  1    transaction_id = TRANSACTION_ID_HEADSET (0xFF)
/// Byte  5    data_size      = 0x01  (1 argument byte)
/// Byte  6    command_class  = CMD_CLASS_SENSOR (0x04)
/// Byte  7    command_id     = CMD_ID_SET_SIDETONE (0x04)
/// Byte  8    args[0]        = level (0-100)
/// Byte 88    crc            = XOR(bytes[2..88])
/// ```
///
/// ⚠️  Command IDs based on Kraken V3 baseline — verify with Wireshark on V4 Pro.
pub fn build_set_sidetone_payload(level: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = TRANSACTION_ID_HEADSET;
    buf[5] = 0x01; // data_size: 1 argument byte
    buf[6] = CMD_CLASS_SENSOR; // 0x04
    buf[7] = CMD_ID_SET_SIDETONE; // 0x04
    buf[8] = level;
    calculate_razer_checksum(&mut buf);
    buf
}

/// Builds a 90-byte Razer HID report that sets the haptic feedback intensity.
///
/// Level is placed directly at args[0] (byte 8), range 0–100.
/// Level 0 disables haptics; any non-zero value sets intensity.
///
/// ```text
/// Byte  1    transaction_id = TRANSACTION_ID_HEADSET (0xFF)
/// Byte  5    data_size      = 0x01  (1 argument byte)
/// Byte  6    command_class  = CMD_CLASS_SENSOR (0x04)
/// Byte  7    command_id     = CMD_ID_SET_HAPTIC_INTENSITY (0x07)
/// Byte  8    args[0]        = level (0-100)
/// Byte 88    crc            = XOR(bytes[2..88])
/// ```
///
/// ⚠️  Command IDs based on Kraken V3 HyperSense baseline — verify with Wireshark.
pub fn build_set_haptic_payload(level: u8) -> [u8; REPORT_LEN] {
    let mut buf = [0u8; REPORT_LEN];
    buf[1] = TRANSACTION_ID_HEADSET;
    buf[5] = 0x01; // data_size: 1 argument byte
    buf[6] = CMD_CLASS_SENSOR; // 0x04
    buf[7] = CMD_ID_SET_HAPTIC_INTENSITY; // 0x07
    buf[8] = level;
    calculate_razer_checksum(&mut buf);
    buf
}

// ── Kraken V4 Pro OLED Hub — 64-byte proprietary HID report ──────────────────
//
// Wireshark capture on Windows confirmed the Kraken V4 Pro Hub (PID 0x0568)
// uses a 64-byte report on Interface 4, NOT the legacy 90-byte Razer protocol.
//
// Wireshark-verified USB Setup Packet (haptics_synapse.pcapng):
//   bmRequestType = 0x21 (HOST→DEVICE | CLASS | INTERFACE)
//   bRequest      = 0x09 (HID SET_REPORT)
//   wValue        = 0x0202 (Output Report ID 2)
//   wIndex        = 0x0004 (Interface 4)
//   wLength       = 64

/// Length of the Kraken V4 Pro Hub proprietary HID report (bytes).
pub const HAPTIC_REPORT_LEN: usize = 64;

/// `cc` and `ci` bytes of Kraken V4 Pro full status push / query packets.
/// Confirmed from Wireshark `sidehaptics.pcapng` interrupt IN responses.
pub const HEADSET_STATUS_CC: u8 = 0x25;
pub const HEADSET_STATUS_CI: u8 = 0x16;

/// `cc` and `ci` bytes of Kraken V4 Pro **spontaneous device-status** packets.
///
/// These arrive automatically on ep=0x84 every poll cycle without being requested.
/// Confirmed from `RUST_LOG=debug` output: `02 02 60 00 00 00 05 00 80 80 20 02 02 00 00 00`
/// Note: byte[8]=0x80 may be a status flag (bit7=connected), NOT a real battery percentage.
/// Kept as a lower-priority fallback after the standard cc=0x07/ci=0x80 query.
pub const HEADSET_DEVICE_STATUS_CC: u8 = 0x05;
pub const HEADSET_DEVICE_STATUS_CI: u8 = 0x00;

/// Standard Razer battery query command class / command ID.
///
/// Confirmed from the C driver `razerchromacommon.c`:
///   `razer_chroma_misc_get_battery_level()` → `get_razer_report(0x07, 0x80, 0x02)`
///   Response: `response.arguments[1]` = battery 0–100 (NOT 0–255 like push packets).
///
/// In the 64-byte HID layout (report_id at byte[0]):
///   byte[6]  = 0x07  cc (HEADSET_BATTERY_CC)
///   byte[7]  = 0x80  ci (HEADSET_BATTERY_CI)
///   byte[10] = arguments[1] = battery percentage (0–100 directly)
pub const HEADSET_BATTERY_CC: u8 = 0x07;
pub const HEADSET_BATTERY_CI: u8 = 0x80;

/// Minimum data_size for a `cc=0x25/ci=0x16` packet to be treated as a full status push.
/// Wireshark push has data_size=0x5f (95). Query ACK has data_size=0x01.
/// Anything below this threshold is an ACK, not a battery status.
const HEADSET_STATUS_MIN_DATA_SIZE: u8 = 0x08;

/// Session-global counter incremented on every haptic send.
/// The headset uses this to sequence multi-packet updates.
static HAPTIC_COUNTER: AtomicU8 = AtomicU8::new(9);

/// Parses a 64-byte Kraken V4 Pro interrupt-IN packet received on ep=0x84.
///
/// Handles three packet formats, checked in priority order:
///
/// **Format C — standard battery query response (`cc=0x07/ci=0x80`, highest priority):**
/// Response to `get_razer_report(0x07, 0x80, 0x02)`. Battery is in `byte[10]` (arguments[1])
/// on a 0–100 scale directly (confirmed from `razermouse_driver.c`: `response.arguments[1]`).
/// ```text
/// byte[6]  = 0x07  cc (HEADSET_BATTERY_CC)
/// byte[7]  = 0x80  ci (HEADSET_BATTERY_CI)
/// byte[10] = battery percentage 0–100 (arguments[1])
/// ```
///
/// **Format B — full status push (`cc=0x25/ci=0x16`):**
/// Only seen during active Synapse sessions (e.g. after haptic commands).
/// Requires `byte[8] >= HEADSET_STATUS_MIN_DATA_SIZE` to distinguish from 1-byte ACKs.
/// Battery raw (0–255) is in `byte[9]`.
/// ```text
/// byte[6] = 0x25  cc (HEADSET_STATUS_CC)
/// byte[7] = 0x16  ci (HEADSET_STATUS_CI)
/// byte[8] = data_size (must be >= 0x08 for a real push; 0x01 = query ACK)
/// byte[9] = raw battery level (0–255 scale → *100/255 = %)
/// ```
///
/// **Format A — spontaneous device-status (`cc=0x05/ci=0x00`, lowest priority fallback):**
/// Arrives automatically every poll cycle. Battery raw (0–255) is in `byte[8]`.
/// ⚠️  `byte[8]=0x80` appears fixed across all observed packets; may be a status flag,
/// not a real battery percentage. Only used when Format C and B are unavailable.
/// ```text
/// byte[6] = 0x05  cc (HEADSET_DEVICE_STATUS_CC)
/// byte[7] = 0x00  ci (HEADSET_DEVICE_STATUS_CI)
/// byte[8] = raw battery level (0–255 scale → *100/255 = %)
/// ```
///
/// Returns `None` when the packet does not contain a valid battery reading.
pub fn parse_headset_push_packet(resp: &[u8; HAPTIC_REPORT_LEN]) -> Option<u8> {
    if resp[6] == HEADSET_BATTERY_CC && resp[7] == HEADSET_BATTERY_CI {
        // Format C: cc=0x07/ci=0x80 — standard battery query response
        // Battery in arguments[1] = byte[10], already 0-100 scale
        let pct = resp[10];
        if pct == 0 || pct > 100 {
            return None; // 0 = no data; >100 = garbled / not a real response
        }
        return Some(pct);
    }

    let raw = if resp[6] == HEADSET_DEVICE_STATUS_CC && resp[7] == HEADSET_DEVICE_STATUS_CI {
        // Format A: cc=0x05/ci=0x00 — battery is in byte[8]
        resp[8]
    } else if resp[6] == HEADSET_STATUS_CC && resp[7] == HEADSET_STATUS_CI {
        // Format B: cc=0x25/ci=0x16 — only accept full pushes (data_size >= min)
        if resp[8] < HEADSET_STATUS_MIN_DATA_SIZE {
            return None; // 1-byte ACK, not a real status push
        }
        resp[9]
    } else {
        return None;
    };

    if raw == 0x00 || raw == 0xFF {
        return None;
    }
    Some(((raw as u16 * 100) / 255) as u8)
}

/// Builds a 64-byte HID query that asks the Kraken V4 Pro to emit a status
/// push packet (`cc=0x25 / ci=0x16`) containing the battery level.
///
/// Sending this via SET_REPORT (wValue=0x0202, wIndex=iface) may prompt the
/// device to respond on ep=0x84 with a status packet parseable by
/// [`parse_headset_push_packet`].
pub fn build_headset_status_query() -> [u8; HAPTIC_REPORT_LEN] {
    let mut buf = [0u8; HAPTIC_REPORT_LEN];
    buf[0] = 0x02; // report_id
    buf[2] = 0x60; // transaction_id (fixed across all Kraken V4 Pro commands)
    buf[6] = HEADSET_STATUS_CC; // 0x25
    buf[7] = HEADSET_STATUS_CI; // 0x16
    buf[8] = 0x01; // data_size (request 1 byte)
    buf
}

/// Builds a 64-byte HID query for the **standard Razer battery level** request.
///
/// Adapts `get_razer_report(0x07, 0x80, 0x02)` (confirmed in `razerchromacommon.c`)
/// to the 64-byte HID packet format used by the Kraken V4 Pro headset.
///
/// Send via SET_REPORT on interface 4 (wIndex=0x0004), then read the response
/// from ep=0x84 and pass to [`parse_headset_push_packet`].
pub fn build_headset_battery_query() -> [u8; HAPTIC_REPORT_LEN] {
    let mut buf = [0u8; HAPTIC_REPORT_LEN];
    buf[0] = 0x02; // report_id
    buf[2] = 0x60; // transaction_id
    buf[6] = HEADSET_BATTERY_CC; // 0x07
    buf[7] = HEADSET_BATTERY_CI; // 0x80
    buf[8] = 0x02; // data_size = 2 arguments
    buf
}
/// Builds a 64-byte HID output report that sets the Kraken V4 Pro haptic intensity.
///
/// **Wireshark-verified layout** captured from Razer Synapse 3 on Windows
/// (`haptics_synapse.pcapng`, `sidehaptics.pcapng`, `volume_sidetone.pcapng`):
///
/// ```text
/// Byte  0     Report ID      = 0x02  (matches wValue low-byte)
/// Byte  1     status         = 0x00
/// Byte  2     transaction_id = 0x60  (fixed)
/// Bytes 3–5   reserved       = 0x00
/// Byte  6     cmd_class      = 0x28
/// Byte  7     cmd_id         = 0x17
/// Byte  8     = 0x09  (fixed)
/// Byte  9     = 0x01  (fixed)
/// Byte 10     haptic_a       — primary intensity (0=off, 78=max observed)
/// Bytes 11–13 = 0x00
/// Byte 14     = 0x02  (field marker)
/// Byte 15     = 0x00
/// Byte 16     haptic_b       — secondary intensity (0=off, 81=max observed)
/// Bytes 17–18 = 0x00
/// Byte 19     = 0x03  (field marker)
/// Bytes 20–23 = 0x00
/// Byte 24     = 0x04  (field marker)
/// Byte 25     = 0x00
/// Byte 26     = 0x3A  (fixed per-session value; narrow observed range 57–58)
/// Bytes 27–28 = 0x00
/// Byte 29     = 0x05  (field marker)
/// Bytes 30–32 = 0x00
/// Byte 33     = 0x06  (field marker)
/// Bytes 34–37 = 0x00
/// Byte 38     = 0x07  (field marker)
/// Byte 39     = 0x09  (fixed)
/// Byte 40     = 0x09  (fixed)
/// Byte 41     = 0x00  (per-session; observed 0x10–0x20 across captures)
/// Byte 42     counter        — increments by 1 each send (AtomicU8, wraps 255→0)
/// Byte 43     = 0x01  (fixed)
/// Byte 44     = 0x08  (fixed)
/// Bytes 45–63 = 0x00  (padding; NO checksum)
/// ```
///
/// Level mapping derived from captures (UI values: 0, 33, 66, 100):
///
/// | level | byte[10] | byte[16] |
/// |-------|----------|----------|
/// |   0   |    0     |    0     |
/// |  33   |   26     |   62     |
/// |  66   |   40     |   68     |
/// |  100  |   78     |   81     |
pub fn build_haptic_report(level: u8) -> [u8; HAPTIC_REPORT_LEN] {
    let mut buf = [0u8; HAPTIC_REPORT_LEN];

    // ── Header ────────────────────────────────────────────────────────────────
    buf[0] = 0x02; // report_id
    buf[2] = 0x60; // transaction_id (fixed)
    buf[6] = 0x28; // cmd_class
    buf[7] = 0x17; // cmd_id

    // ── Intensity ─────────────────────────────────────────────────────────────
    let (haptic_a, haptic_b): (u8, u8) = if level == 0 {
        (0, 0)
    } else {
        let a = (u16::from(level) * 78 / 100) as u8;
        let b = (60u16 + u16::from(level) * 21 / 100) as u8;
        (a.max(1), b)
    };

    buf[8] = 0x09;
    buf[9] = 0x01;
    buf[10] = haptic_a;
    buf[14] = 0x02;
    buf[16] = haptic_b;
    buf[19] = 0x03;
    buf[24] = 0x04;
    buf[26] = 0x3A; // per-session fixed value
    buf[29] = 0x05;
    buf[33] = 0x06;
    buf[38] = 0x07;
    buf[39] = 0x09;
    buf[40] = 0x09;
    buf[42] = HAPTIC_COUNTER.fetch_add(1, Ordering::Relaxed);
    buf[43] = 0x01;
    buf[44] = 0x08;

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact 90-byte match for DeathAdder V2 Pro params, Red (0xFF, 0x00, 0x00).
    /// CRC = 0x09^0x0F^0x02^0x01^0x05^0x01^0x01^0xFF = 0xFF
    #[test]
    fn test_static_color_payload_generation() {
        let mut expected = [0u8; 90];
        expected[1] = TRANSACTION_ID_DA; // 0x3F
        expected[5] = 0x09;
        expected[6] = 0x0F;
        expected[7] = 0x02;
        expected[8] = 0x01; // VARSTORE
        expected[9] = LED_BACKLIGHT; // 0x05
        expected[10] = 0x01; // STATIC
        expected[13] = 0x01; // colour count
        expected[14] = 0xFF; // R
        expected[88] = 0xFF; // CRC

        let got = build_static_color_payload(TRANSACTION_ID_DA, LED_BACKLIGHT, 0xFF, 0x00, 0x00);
        assert_eq!(got, expected, "DA V2 Pro red payload mismatch");
    }

    /// Cobra Pro params (transaction_id=0x1F, led_id=0x00), Red.
    /// CRC = 0x09^0x0F^0x02^0x01^0x00^0x01^0x01^0xFF = 0xFA
    #[test]
    fn test_static_color_payload_cobra_pro() {
        let payload = build_static_color_payload(TRANSACTION_ID_COBRA, LED_ZERO, 0xFF, 0x00, 0x00);
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[9], LED_ZERO, "led_id mismatch");
        assert_eq!(payload[14], 0xFF, "R mismatch");
        assert_eq!(payload[88], 0xFA, "CRC mismatch");
    }

    /// Pure-black CRC for Cobra Pro params.
    /// CRC = 0x09^0x0F^0x02^0x01^0x00^0x01^0x01 = 0x05
    #[test]
    fn test_static_color_payload_black() {
        let payload = build_static_color_payload(TRANSACTION_ID_COBRA, LED_ZERO, 0x00, 0x00, 0x00);
        assert_eq!(payload[88], 0x05, "CRC should be 0x05 for Cobra Pro black");
        assert_eq!(payload[14], 0x00);
        assert_eq!(payload[15], 0x00);
        assert_eq!(payload[16], 0x00);
    }

    /// Struct layout spot-check with Cobra Pro params.
    #[test]
    fn test_static_color_payload_layout() {
        let payload = build_static_color_payload(TRANSACTION_ID_COBRA, LED_ZERO, 0x11, 0x22, 0x33);
        assert_eq!(payload[0], 0x00, "status must be 0x00");
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x09, "data_size must be 9");
        assert_eq!(payload[6], 0x0F, "command_class must be 0x0F");
        assert_eq!(payload[7], 0x02, "command_id must be 0x02");
        assert_eq!(payload[9], LED_ZERO, "led zone must be ZERO (0x00)");
        assert_eq!(payload[14], 0x11, "R mismatch");
        assert_eq!(payload[15], 0x22, "G mismatch");
        assert_eq!(payload[16], 0x33, "B mismatch");
        assert_eq!(payload[89], 0x00, "reserved byte must be 0x00");
    }

    // ── Battery query tests ───────────────────────────────────────────────────

    /// Battery level query for Cobra Pro (transaction_id=0x1F).
    /// CRC = 0x02 ^ 0x07 ^ 0x80 = 0x85
    #[test]
    fn test_battery_query_payload_cobra_pro() {
        let payload = build_battery_query_payload(TRANSACTION_ID_COBRA);
        assert_eq!(payload[0], 0x00, "status must be 0x00");
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x02, "data_size must be 2");
        assert_eq!(payload[6], CMD_CLASS_BATTERY, "command_class must be 0x07");
        assert_eq!(payload[7], CMD_ID_BATTERY_LEVEL, "command_id must be 0x80");
        assert_eq!(payload[89], 0x00, "reserved byte must be 0x00");
        // CRC: 0x02 ^ 0x07 ^ 0x80 = 0x85
        assert_eq!(payload[88], 0x85, "CRC mismatch");
    }

    /// Charging status query for Cobra Pro (transaction_id=0x1F).
    /// CRC = 0x02 ^ 0x07 ^ 0x84 = 0x81
    #[test]
    fn test_charging_query_payload_cobra_pro() {
        let payload = build_charging_query_payload(TRANSACTION_ID_COBRA);
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x02, "data_size must be 2");
        assert_eq!(payload[6], CMD_CLASS_BATTERY, "command_class must be 0x07");
        assert_eq!(
            payload[7], CMD_ID_CHARGING_STATUS,
            "command_id must be 0x84"
        );
        // CRC: 0x02 ^ 0x07 ^ 0x84 = 0x81
        assert_eq!(payload[88], 0x81, "CRC mismatch");
    }

    /// DA V2 Pro uses a different transaction_id (0x3F); CRC must reflect that.
    /// CRC = 0x02 ^ 0x07 ^ 0x80 = 0x85  (transaction_id is byte[1], outside CRC range)
    #[test]
    fn test_battery_query_payload_da_v2_pro() {
        let payload = build_battery_query_payload(TRANSACTION_ID_DA);
        assert_eq!(payload[1], TRANSACTION_ID_DA, "transaction_id mismatch");
        assert_eq!(payload[6], CMD_CLASS_BATTERY);
        assert_eq!(payload[7], CMD_ID_BATTERY_LEVEL);
        // CRC bytes [2..88] are identical to Cobra Pro — transaction_id at [1] is outside range
        assert_eq!(
            payload[88], 0x85,
            "CRC should be same regardless of transaction_id"
        );
    }

    /// Kraken V4 Pro Static Red (255, 0, 0).
    /// Non-zero bytes in [2..88]: [5]=0x05, [6]=0x0F, [7]=0x02, [8]=0xFF
    /// CRC = 0x05 ^ 0x0F ^ 0x02 ^ 0xFF = 0xF7
    #[test]
    fn test_kraken_v4_static_payload_red() {
        let mut expected = [0u8; 90];
        expected[1] = 0xFF;
        expected[5] = 0x05;
        expected[6] = 0x0F;
        expected[7] = 0x02;
        expected[8] = 0xFF; // R
        expected[9] = 0x00; // G
        expected[10] = 0x00; // B
        expected[88] = 0xF7; // Pre-calculated XOR

        let got = build_kraken_v4_static_payload(0xFF, 0x00, 0x00);
        assert_eq!(got, expected, "Kraken V4 Pro red payload mismatch");
    }

    // ── Breathing effect tests ────────────────────────────────────────────────

    /// Breathing Single, Cobra Pro params (txn=0x1F, led=0x00), Red (0xFF,0,0).
    /// Non-zero bytes [2..88]: [5]=0x09, [6]=0x0F, [7]=0x02, [8]=0x01,
    ///   [9]=0x00(LED_ZERO), [10]=0x02, [13]=0x01(count), [14]=0xFF(R)
    /// CRC = 0x09^0x0F^0x02^0x01^0x02^0x01^0xFF = 0xF9
    #[test]
    fn test_breathing_payload_cobra_pro_red() {
        let payload = build_breathing_payload(TRANSACTION_ID_COBRA, LED_ZERO, 0xFF, 0x00, 0x00);
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x09, "data_size must be 9");
        assert_eq!(payload[6], CMD_CLASS);
        assert_eq!(payload[7], CMD_ID);
        assert_eq!(payload[8], VARSTORE);
        assert_eq!(payload[9], LED_ZERO);
        assert_eq!(payload[10], EFFECT_BREATHING_SINGLE);
        assert_eq!(payload[13], 0x01, "colour count must be 1");
        assert_eq!(payload[14], 0xFF, "R mismatch");
        assert_eq!(payload[15], 0x00, "G mismatch");
        assert_eq!(payload[16], 0x00, "B mismatch");
        assert_eq!(payload[88], 0xF9, "CRC mismatch");
    }

    // ── Spectrum effect tests ─────────────────────────────────────────────────

    /// Spectrum, Cobra Pro params (txn=0x1F, led=0x00).
    /// Non-zero bytes [2..88]: [5]=0x03, [6]=0x0F, [7]=0x02, [8]=0x01, [10]=0x04
    /// CRC = 0x03^0x0F^0x02^0x01^0x04 = 0x0B
    #[test]
    fn test_spectrum_payload_cobra_pro() {
        let payload = build_spectrum_payload(TRANSACTION_ID_COBRA, LED_ZERO);
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x03, "data_size must be 3");
        assert_eq!(payload[6], CMD_CLASS);
        assert_eq!(payload[7], CMD_ID);
        assert_eq!(payload[8], VARSTORE);
        assert_eq!(payload[9], LED_ZERO);
        assert_eq!(payload[10], EFFECT_SPECTRUM);
        // RGB bytes must be zero — spectrum carries no colour data
        assert_eq!(payload[14], 0x00);
        assert_eq!(payload[15], 0x00);
        assert_eq!(payload[16], 0x00);
        assert_eq!(payload[88], 0x0B, "CRC mismatch");
    }

    // ── Battery payload + parse round-trip ────────────────────────────────────

    /// Verifies that `build_battery_query_payload` generates a correct HID
    /// report AND that `parse_battery_response` correctly converts the raw
    /// 0–255 hardware value at index 9 into a 0–100 % integer.
    #[test]
    fn test_battery_payload_and_parse() {
        // ── Payload generation ────────────────────────────────────────────
        let payload = build_battery_query_payload(TRANSACTION_ID_COBRA);
        assert_eq!(payload[5], 0x02, "data_size must be 2");
        assert_eq!(payload[6], CMD_CLASS_BATTERY, "command class must be 0x07");
        assert_eq!(payload[7], CMD_ID_BATTERY_LEVEL, "command id must be 0x80");

        // ── Parse: 255 raw → 100 % ────────────────────────────────────────
        let mut response = [0u8; REPORT_LEN];
        response[9] = 255;
        assert_eq!(parse_battery_response(&response), 100);

        // ── Parse: 128 raw → 50 % (integer: 128*100/255 = 50) ────────────
        response[9] = 128;
        assert_eq!(parse_battery_response(&response), 50);

        // ── Parse: 0 raw → 0 % ───────────────────────────────────────────
        response[9] = 0;
        assert_eq!(parse_battery_response(&response), 0);

        // ── Parse: 191 raw → 74 % (191*100/255 = 74) ─────────────────────
        response[9] = 191;
        assert_eq!(parse_battery_response(&response), 74);
    }

    // ── DPI payload tests ─────────────────────────────────────────────────────

    /// 800 DPI on both axes for Cobra Pro (txn_id = 0x1F).
    ///
    /// 800 = 0x0320  →  high = 0x03, low = 0x20
    ///
    /// Non-zero bytes in [2..88]:
    ///   [5]=0x07, [6]=0x04, [7]=0x05, [8]=0x01(VARSTORE),
    ///   [9]=0x03, [10]=0x20, [11]=0x03, [12]=0x20
    ///
    /// CRC = 0x07 ^ 0x04 ^ 0x05 ^ 0x01 ^ 0x03 ^ 0x20 ^ 0x03 ^ 0x20 = 0x07
    #[test]
    fn test_set_dpi_payload_800_800() {
        let payload = build_set_dpi_payload(TRANSACTION_ID_COBRA, 800, 800);

        assert_eq!(payload[0], 0x00, "status must be 0x00");
        assert_eq!(payload[1], TRANSACTION_ID_COBRA, "transaction_id mismatch");
        assert_eq!(payload[5], 0x07, "data_size must be 7");
        assert_eq!(payload[6], CMD_CLASS_SENSOR, "command_class must be 0x04");
        assert_eq!(payload[7], CMD_ID_SET_DPI, "command_id must be 0x05");
        assert_eq!(payload[8], VARSTORE, "args[0] must be VARSTORE");
        // dpi_x = 800 = 0x0320
        assert_eq!(payload[9], 0x03, "dpi_x high byte mismatch");
        assert_eq!(payload[10], 0x20, "dpi_x low byte mismatch");
        // dpi_y = 800 = 0x0320
        assert_eq!(payload[11], 0x03, "dpi_y high byte mismatch");
        assert_eq!(payload[12], 0x20, "dpi_y low byte mismatch");
        assert_eq!(payload[88], 0x07, "CRC mismatch");
        assert_eq!(payload[89], 0x00, "reserved byte must be 0x00");
    }

    /// Asymmetric DPI: X=1600 (0x0640), Y=400 (0x0190) for DA V2 Pro.
    ///
    /// CRC = 0x07 ^ 0x04 ^ 0x05 ^ 0x01 ^ 0x06 ^ 0x40 ^ 0x01 ^ 0x90 = 0xD4
    #[test]
    fn test_set_dpi_payload_asymmetric() {
        let payload = build_set_dpi_payload(TRANSACTION_ID_DA, 1600, 400);

        // 1600 = 0x0640
        assert_eq!(payload[9], 0x06, "dpi_x high (1600)");
        assert_eq!(payload[10], 0x40, "dpi_x low (1600)");
        // 400 = 0x0190
        assert_eq!(payload[11], 0x01, "dpi_y high (400)");
        assert_eq!(payload[12], 0x90, "dpi_y low (400)");
        assert_eq!(payload[88], 0xD0, "CRC mismatch for asymmetric DPI");
    }

    // ── Headset audio / haptic payload tests ─────────────────────────────────

    /// Sidetone at level=50 (0x32).
    ///
    /// Non-zero bytes in [2..88]: [5]=0x01, [6]=0x04, [7]=0x04, [8]=0x32
    /// CRC = 0x01 ^ 0x04 ^ 0x04 ^ 0x32 = 0x33
    /// (byte[1]=0xFF is the transaction_id and lies outside the XOR range [2..88])
    #[test]
    fn test_sidetone_payload_generation() {
        let payload = build_set_sidetone_payload(50);

        assert_eq!(payload[0], 0x00, "status must be 0x00 (new command)");
        assert_eq!(
            payload[1], TRANSACTION_ID_HEADSET,
            "transaction_id must be 0xFF"
        );
        assert_eq!(payload[5], 0x01, "data_size must be 1 for sidetone");
        assert_eq!(
            payload[6], CMD_CLASS_SENSOR,
            "command_class must be 0x04 (audio)"
        );
        assert_eq!(
            payload[7], CMD_ID_SET_SIDETONE,
            "command_id must be 0x04 (sidetone)"
        );
        assert_eq!(payload[8], 50, "level byte must be at args[0] (byte 8)");
        assert_eq!(payload[89], 0x00, "reserved byte must be 0x00");

        // Verify checksum is self-consistent.
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(recomputed, payload[88], "XOR checksum mismatch");
        assert_eq!(payload[88], 0x33, "CRC mismatch for sidetone level=50");
    }

    /// Sidetone at level=0 should still form a valid payload (silence).
    #[test]
    fn test_sidetone_payload_zero_level() {
        let payload = build_set_sidetone_payload(0);
        assert_eq!(payload[8], 0x00, "level byte must be 0");
        // CRC = 0x01 ^ 0x04 ^ 0x04 = 0x01
        assert_eq!(payload[88], 0x01, "CRC mismatch for sidetone level=0");
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(recomputed, payload[88]);
    }

    /// Haptic intensity at level=75 (0x4B).
    ///
    /// Non-zero bytes in [2..88]: [5]=0x01, [6]=0x04, [7]=0x07, [8]=0x4B
    /// CRC = 0x01 ^ 0x04 ^ 0x07 ^ 0x4B = 0x49
    #[test]
    fn test_haptic_payload_generation() {
        let payload = build_set_haptic_payload(75);

        assert_eq!(payload[0], 0x00, "status must be 0x00 (new command)");
        assert_eq!(
            payload[1], TRANSACTION_ID_HEADSET,
            "transaction_id must be 0xFF"
        );
        assert_eq!(payload[5], 0x01, "data_size must be 1 for haptics");
        assert_eq!(
            payload[6], CMD_CLASS_SENSOR,
            "command_class must be 0x04 (audio)"
        );
        assert_eq!(
            payload[7], CMD_ID_SET_HAPTIC_INTENSITY,
            "command_id must be 0x07 (haptics)"
        );
        assert_eq!(payload[8], 75, "level byte must be at args[0] (byte 8)");
        assert_eq!(payload[9], 0x00, "byte 9 must be unused (0x00)");
        assert_eq!(payload[89], 0x00, "reserved byte must be 0x00");

        // CRC = 0x01 ^ 0x04 ^ 0x07 ^ 0x4B = 0x49
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(recomputed, payload[88], "XOR checksum mismatch");
        assert_eq!(payload[88], 0x49, "CRC mismatch for haptic level=75");
    }

    /// Haptic at level=0 — disables haptics.
    #[test]
    fn test_haptic_payload_zero_level() {
        let payload = build_set_haptic_payload(0);
        assert_eq!(payload[8], 0x00, "level must be 0x00");
        // CRC = 0x01 ^ 0x04 ^ 0x07 = 0x02
        assert_eq!(payload[88], 0x02, "CRC mismatch for haptic level=0");
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(recomputed, payload[88]);
    }

    /// Canonical V3-legacy verification test (spec name): sidetone level=50.
    /// Asserts the exact bytes 6, 7, 8 and a valid XOR checksum.
    #[test]
    fn test_sidetone_payload_v3_legacy() {
        let payload = build_set_sidetone_payload(50);
        assert_eq!(payload[6], 0x04, "command_class must be 0x04");
        assert_eq!(payload[7], 0x04, "command_id must be 0x04 (sidetone)");
        assert_eq!(payload[8], 50, "level must be at index 8");
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(
            recomputed, payload[88],
            "XOR checksum is not mathematically valid"
        );
    }

    /// Canonical V3-legacy verification test (spec name): haptic level=50.
    /// Asserts the exact bytes 6, 7, 8 and a valid XOR checksum.
    #[test]
    fn test_haptic_payload_v3_legacy() {
        let payload = build_set_haptic_payload(50);
        assert_eq!(payload[6], 0x04, "command_class must be 0x04");
        assert_eq!(payload[7], 0x07, "command_id must be 0x07 (haptics)");
        assert_eq!(payload[8], 50, "level must be at index 8");
        let recomputed: u8 = payload[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
        assert_eq!(
            recomputed, payload[88],
            "XOR checksum is not mathematically valid"
        );
    }

    // ── Kraken V4 Pro OLED Hub — 64-byte haptic report ───────────────────────

    /// Verify Wireshark-verified fixed bytes and intensity mapping for level=66.
    #[test]
    fn test_haptic_report_v4_pro_layout() {
        let report = build_haptic_report(66);

        assert_eq!(report.len(), 64, "report must be exactly 64 bytes");
        assert_eq!(report[0], 0x02, "Report ID must be 0x02");
        assert_eq!(report[2], 0x60, "transaction_id must be 0x60");
        assert_eq!(report[6], 0x28, "cmd_class must be 0x28");
        assert_eq!(report[7], 0x17, "cmd_id must be 0x17");
        assert_eq!(report[8], 0x09, "fixed byte[8] must be 0x09");
        assert_eq!(report[9], 0x01, "fixed byte[9] must be 0x01");
        assert_eq!(report[14], 0x02, "field marker byte[14] must be 0x02");
        assert_eq!(report[19], 0x03, "field marker byte[19] must be 0x03");
        assert_eq!(report[24], 0x04, "field marker byte[24] must be 0x04");
        assert_eq!(report[29], 0x05, "field marker byte[29] must be 0x05");
        assert_eq!(report[33], 0x06, "field marker byte[33] must be 0x06");
        assert_eq!(report[38], 0x07, "field marker byte[38] must be 0x07");
        assert_eq!(report[39], 0x09, "fixed byte[39] must be 0x09");
        assert_eq!(report[40], 0x09, "fixed byte[40] must be 0x09");
        assert_eq!(report[43], 0x01, "fixed byte[43] must be 0x01");
        assert_eq!(report[44], 0x08, "fixed byte[44] must be 0x08");

        // Level 66 → haptic_a = 66*78/100 = 51, haptic_b = 60 + 66*21/100 = 73
        assert_eq!(report[10], 51, "haptic_a for level=66");
        assert_eq!(report[16], 73, "haptic_b for level=66");

        // No checksum — trailing bytes must all be zero
        assert_eq!(report[62], 0x00, "no checksum: byte[62] must be 0x00");
        assert_eq!(report[63], 0x00, "padding: byte[63] must be 0x00");
    }

    /// Intensity 0 must zero out haptic_a and haptic_b.
    #[test]
    fn test_haptic_report_v4_pro_disable() {
        let report = build_haptic_report(0);
        assert_eq!(report[10], 0x00, "haptic_a must be 0x00 when disabled");
        assert_eq!(report[16], 0x00, "haptic_b must be 0x00 when disabled");
        // All padding bytes should remain zero
        assert_eq!(report[62], 0x00, "no checksum: byte[62] must be 0x00");
    }

    /// Level 100 produces the maximum observed byte values.
    #[test]
    fn test_haptic_report_v4_pro_max_intensity() {
        let report = build_haptic_report(100);
        // haptic_a = 100*78/100 = 78, haptic_b = 60 + 100*21/100 = 81
        assert_eq!(report[10], 78, "haptic_a at max level");
        assert_eq!(report[16], 81, "haptic_b at max level");
    }

    // ── Kraken V4 Pro headset status push packet tests ────────────────────────
    //
    // Byte arrays taken verbatim from Wireshark `sidehaptics.pcapng`.

    /// Valid status push: byte[9]=0x80 → (128 * 100) / 255 = 50 %
    ///
    /// Full first-16-bytes from Wireshark:
    ///   02 02 60 00 00 00 25 16 5f 80 8b 01 01 01 06 00
    #[test]
    fn test_parse_headset_push_valid_wireshark() {
        let mut resp = [0u8; HAPTIC_REPORT_LEN];
        resp[0] = 0x02; // report_id
        resp[1] = 0x02; // status=success
        resp[2] = 0x60; // txn_id
        resp[6] = HEADSET_STATUS_CC; // 0x25
        resp[7] = HEADSET_STATUS_CI; // 0x16
        resp[8] = 0x5f; // data_size=95
        resp[9] = 0x80; // raw=128 → 50%
        resp[10] = 0x8b;
        resp[11] = 0x01;
        resp[12] = 0x01;
        resp[13] = 0x01;
        resp[14] = 0x06;

        let pct = parse_headset_push_packet(&resp);
        assert_eq!(pct, Some(50), "0x80 (128) should decode as 50%");
    }

    /// The haptic ACK packet (cc=0x00/ci=0x17) must be rejected.
    ///
    /// Full first-16-bytes from Wireshark:
    ///   02 02 60 00 00 00 00 17 09 00 00 00 00 00 00 00
    #[test]
    fn test_parse_headset_push_wrong_cc_haptic_ack() {
        let mut resp = [0u8; HAPTIC_REPORT_LEN];
        resp[0] = 0x02;
        resp[1] = 0x02;
        resp[2] = 0x60;
        resp[6] = 0x00; // cc=0x00 (haptic ACK, NOT a status push)
        resp[7] = 0x17; // ci=0x17
        resp[8] = 0x09;
        // All data bytes remain 0x00

        assert_eq!(
            parse_headset_push_packet(&resp),
            None,
            "haptic ACK (cc=0x00/ci=0x17) must not be decoded as battery"
        );
    }

    /// Packets with raw==0x00 (device returned zeros) must be rejected.
    #[test]
    fn test_parse_headset_push_raw_zero_rejected() {
        let mut resp = [0u8; HAPTIC_REPORT_LEN];
        resp[6] = HEADSET_STATUS_CC;
        resp[7] = HEADSET_STATUS_CI;
        resp[9] = 0x00; // raw=0 → invalid

        assert_eq!(
            parse_headset_push_packet(&resp),
            None,
            "raw=0x00 must be rejected"
        );
    }

    /// Packets with raw==0xFF (all-ones sentinel) must be rejected.
    #[test]
    fn test_parse_headset_push_raw_ff_rejected() {
        let mut resp = [0u8; HAPTIC_REPORT_LEN];
        resp[6] = HEADSET_STATUS_CC;
        resp[7] = HEADSET_STATUS_CI;
        resp[9] = 0xFF; // raw=255 → invalid sentinel

        assert_eq!(
            parse_headset_push_packet(&resp),
            None,
            "raw=0xFF must be rejected"
        );
    }

    /// Verify `build_headset_status_query` byte layout.
    /// We expect: byte[0]=0x02, byte[2]=0x60, byte[6]=0x25, byte[7]=0x16, byte[8]=0x01.
    /// All other bytes must be 0x00.
    #[test]
    fn test_build_headset_status_query_layout() {
        let q = build_headset_status_query();
        assert_eq!(q.len(), HAPTIC_REPORT_LEN, "must be exactly 64 bytes");
        assert_eq!(q[0], 0x02, "report_id must be 0x02");
        assert_eq!(q[1], 0x00, "status must be 0x00 (host→device)");
        assert_eq!(q[2], 0x60, "transaction_id must be 0x60");
        assert_eq!(q[6], HEADSET_STATUS_CC, "cc must be 0x25");
        assert_eq!(q[7], HEADSET_STATUS_CI, "ci must be 0x16");
        assert_eq!(q[8], 0x01, "data_size must be 0x01");
        // All other bytes must be zero (no stray bytes)
        for i in [3, 4, 5, 9, 10, 62, 63] {
            assert_eq!(q[i], 0x00, "byte[{i}] must be 0x00");
        }
    }

    /// cc=0x05/ci=0x00 spontaneous status packet containing battery in byte[8].
    ///
    /// Exact bytes observed in RUST_LOG=debug output every poll cycle:
    ///   02 02 60 00 00 00 05 00 80 80 20 02 02 00 00 00
    /// byte[8]=0x80=128 → 128*100/255 ≈ 50%
    #[test]
    fn test_parse_headset_0x05_packet_with_battery() {
        let mut pkt = [0u8; HAPTIC_REPORT_LEN];
        pkt[0] = 0x02; // report_id
        pkt[1] = 0x02; // status=success
        pkt[2] = 0x60; // txn_id
        pkt[6] = HEADSET_DEVICE_STATUS_CC; // 0x05
        pkt[7] = HEADSET_DEVICE_STATUS_CI; // 0x00
        pkt[8] = 0x80; // raw battery = 128 → 50%
        pkt[9] = 0x80;
        pkt[10] = 0x20;
        pkt[11] = 0x02;
        pkt[12] = 0x02;

        let pct = parse_headset_push_packet(&pkt);
        assert_eq!(
            pct,
            Some(50),
            "0x80 raw in cc=0x05 packet should decode as 50%"
        );
    }

    /// cc=0x05/ci=0x00 packet with byte[8]=0x00 (zero battery) must be rejected.
    #[test]
    fn test_parse_headset_0x05_packet_zero_rejected() {
        let mut pkt = [0u8; HAPTIC_REPORT_LEN];
        pkt[6] = HEADSET_DEVICE_STATUS_CC; // 0x05
        pkt[7] = HEADSET_DEVICE_STATUS_CI; // 0x00
        pkt[8] = 0x00; // zero → invalid

        assert_eq!(
            parse_headset_push_packet(&pkt),
            None,
            "cc=0x05 with byte[8]=0x00 must be rejected"
        );
    }

    /// cc=0x25/ci=0x16 with data_size=0x01 (1-byte ACK, byte[9]=0x00) must be rejected.
    ///
    /// Exact bytes observed in RUST_LOG=debug every poll cycle (query ACK, not a full push):
    ///   02 02 60 00 00 00 25 16 01 00 00 01 00 00 00 00
    /// byte[8]=0x01 means data_size=1 → this is just an ACK, not a battery status push.
    #[test]
    fn test_parse_headset_push_1byte_ack_rejected() {
        let mut pkt = [0u8; HAPTIC_REPORT_LEN];
        pkt[0] = 0x02;
        pkt[1] = 0x02;
        pkt[2] = 0x60;
        pkt[6] = HEADSET_STATUS_CC; // 0x25
        pkt[7] = HEADSET_STATUS_CI; // 0x16
        pkt[8] = 0x01; // data_size=1 → 1-byte ACK
        pkt[9] = 0x00; // arg[0]=0x00 (no battery data)
        pkt[11] = 0x01;

        assert_eq!(
            parse_headset_push_packet(&pkt),
            None,
            "cc=0x25 1-byte ACK (byte[8]=0x01) must be rejected (not a full status push)"
        );
    }

    /// cc=0x07/ci=0x80 is the standard Razer battery query response format.
    ///
    /// Confirmed from the C driver `razermouse_driver.c`:
    ///   `razer_chroma_misc_get_battery_level()` → `get_razer_report(0x07, 0x80, 0x02)`
    ///   `response.arguments[1]` = battery (0-100 scale, NOT 0-255 unlike status pushes)
    ///
    /// In the 64-byte HID packet layout (with report_id at byte[0]):
    ///   byte[6]  = 0x07  cc
    ///   byte[7]  = 0x80  ci
    ///   byte[8+] = arguments[0..]
    ///   byte[10] = arguments[1] = battery percentage (0–100 directly)
    #[test]
    fn test_parse_headset_standard_battery_query_response() {
        let mut pkt = [0u8; HAPTIC_REPORT_LEN];
        pkt[0] = 0x02; // report_id
        pkt[1] = 0x02; // status=success
        pkt[2] = 0x60; // txn_id
        pkt[6] = HEADSET_BATTERY_CC; // 0x07
        pkt[7] = HEADSET_BATTERY_CI; // 0x80
        pkt[8] = 0x00; // arguments[0] = 0 (unused in battery response)
        pkt[9] = 0x00; // arguments[0] continued
        pkt[10] = 74; // arguments[1] = battery = 74% directly (0-100 scale)

        let pct = parse_headset_push_packet(&pkt);
        assert_eq!(
            pct,
            Some(74),
            "cc=0x07/ci=0x80 standard battery query: byte[10]=74 should decode as 74%"
        );
    }

    /// cc=0x07/ci=0x80 with byte[10]=0 must be rejected (no battery data).
    #[test]
    fn test_parse_headset_standard_battery_query_zero_rejected() {
        let mut pkt = [0u8; HAPTIC_REPORT_LEN];
        pkt[6] = HEADSET_BATTERY_CC; // 0x07
        pkt[7] = HEADSET_BATTERY_CI; // 0x80
        pkt[10] = 0x00;

        assert_eq!(
            parse_headset_push_packet(&pkt),
            None,
            "cc=0x07 with byte[10]=0x00 must be rejected"
        );
    }
}
