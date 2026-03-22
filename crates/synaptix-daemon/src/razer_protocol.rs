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
/// Used by `usb_backend::query_battery` and by the TDD test suite.
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
        _ => {
            eprintln!(
                "[USB] Response status=0x{:02x} for cmd 0x{command_class:02x}/0x{command_id:02x}",
                response[0]
            );
            return Err(true); // hard error
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
}
