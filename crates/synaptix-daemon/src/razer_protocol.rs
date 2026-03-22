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

// ── Per-device transaction IDs ────────────────────────────────────────────────

/// Transaction ID for DeathAdder V2 Pro and similar older wireless mice.
pub const TRANSACTION_ID_DA: u8 = 0x3F;

/// Transaction ID for Cobra Pro, Basilisk V3 Pro, and newer wireless mice.
pub const TRANSACTION_ID_COBRA: u8 = 0x1F;

// ── Per-device LED zone IDs ───────────────────────────────────────────────────

/// Zero / catch-all LED zone (used by Cobra Pro and many newer mice).
pub const LED_ZERO: u8 = 0x00;

/// Backlight LED zone — covers the logo on DA V2 Pro and similar mice.
pub const LED_BACKLIGHT: u8 = 0x05;

// ─────────────────────────────────────────────────────────────────────────────

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

    // CRC: XOR of bytes 2..88 (razercommon.c → razer_calculate_crc).
    buf[88] = buf[2..88].iter().fold(0u8, |acc, &byte| acc ^ byte);

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
}
