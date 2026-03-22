# Adding New Device Support to Synaptix

This guide walks through the full process of adding a new Razer device to Synaptix — from identifying the hardware protocol to writing tests and wiring up the D-Bus interface.

> [!TIP]
> **Before you start:** Read the architecture overview in `README.md`. The golden rule is: hardware logic lives exclusively in `synaptix-daemon`. Never add USB code to the Tauri UI.

---

## Table of Contents

1. [Identify the Device](#1-identify-the-device)
2. [Determine the Protocol Family](#2-determine-the-protocol-family)
3. [Capture USB Traffic](#3-capture-usb-traffic)
4. [Decode the Payload](#4-decode-the-payload)
5. [Update `synaptix-protocol`](#5-update-synaptix-protocol)
6. [Write the Payload Builder (TDD)](#6-write-the-payload-builder-tdd)
7. [Wire Up the Daemon](#7-wire-up-the-daemon)
8. [Update the README](#8-update-the-readme)
9. [Open a Pull Request](#9-open-a-pull-request)

---

## 1. Identify the Device

### Find the USB Vendor/Product IDs

Plug the device in and run:

```bash
lsusb | grep 1532
```

Example output:

```
Bus 003 Device 011: ID 1532:0568 Razer USA, Ltd Razer Kraken V4 Pro
```

Note both values:
- **Vendor ID (VID):** always `0x1532` for Razer
- **Product ID (PID):** device-specific (e.g. `0x0568`)

Some wireless devices expose **two PIDs** — one for wired/charging mode and one for the wireless dongle:

```
Bus 003 Device 013: ID 1532:0567  ← wireless dongle
Bus 003 Device 011: ID 1532:0568  ← wired / charging
```

Add **both** if present.

---

## 2. Determine the Protocol Family

Razer devices do **not** all share the same USB report format. There are currently two families in Synaptix:

| Family | Report Size | `wValue` | `wIndex` | Devices |
|---|---|---|---|---|
| **Mouse / Standard Matrix** | 90 bytes | `0x0300` | `0x0000` | Mice, most peripherals |
| **Kraken / Headset** | 37 bytes | `0x0204` | `0x0003` | Kraken-family headsets |

### How to tell which family your device belongs to

Check the [`openrazer`](https://github.com/openrazer/openrazer) repo for a matching driver file:

```bash
# Mice and keyboards
grep -r "YOUR_PID" https://github.com/openrazer/openrazer/tree/master/driver/razermouse_driver.h
grep -r "YOUR_PID" https://github.com/openrazer/openrazer/tree/master/driver/razerkbd_driver.h

# Headsets
grep -r "YOUR_PID" https://github.com/openrazer/openrazer/tree/master/driver/razerkraken_driver.h
```

If the PID is **not found** in any reference file, your device is new and the protocol must be reverse-engineered (see [Section 3](#3-capture-usb-traffic)).

### Per-device parameters (mice)

For mouse-family devices you also need two per-device values:

| Parameter | Description | Example |
|---|---|---|
| `transaction_id` | Identifies the firmware command channel | `0x1F` (Cobra Pro), `0x3F` (DA V2 Pro) |
| `led_id` | Which LED zone to target | `0x00` (ZERO/Cobra Pro), `0x05` (BACKLIGHT/DA V2 Pro) |

Find these in `razermouse_driver.c` by searching for `transaction_id` near your device's case label.

---

## 3. Capture USB Traffic

> [!NOTE]
> Skip this section if your device's PID and protocol are already documented in `openrazer`.

### Dump HID descriptors

```bash
# Shows interfaces exposed by the device
sudo usbhid-dump -d 1532:YOUR_PID -e descriptor
```

Count the interfaces — this tells you the `wIndex` value to use.

### Capture HID stream events

```bash
# Replace YOUR_PID; timeout is in milliseconds
sudo usbhid-dump -d 1532:YOUR_PID -e stream -t 10000
```

Press buttons on the device while this runs. The output is **input reports** (key presses, sensor data) — useful for confirming the device is communicating.

### Capture lighting control transfers with Wireshark

> [!NOTE]
> Wireshark Installation: [Download and Install Wireshark](https://www.wireshark.org/download.html)

Lighting commands are **USB HID control transfers**, not stream reports. You need `usbmon` to capture them:

```bash
# Load the usbmon kernel module
sudo modprobe usbmon

# Open Wireshark on the correct bus (check lsusb for bus number)
sudo wireshark &
```

In Wireshark:
1. Select the `usbmonX` interface matching your device's bus number
2. Apply the display filter: `usb.idVendor == 0x1532`
3. Use Razer Synapse (Windows VM with USB passthrough, or Wine) to set a static colour
4. Look for `URB_CONTROL out` packets — the `Data Fragment` field contains the raw payload bytes

Screenshot the captured packet bytes — this is your **known-good reference payload**.

---

## 4. Decode the Payload

### Mouse / Standard Matrix (90-byte report)

```
Byte  0     status            = 0x00
Byte  1     transaction_id    (device-specific)
Bytes 2-3   remaining_packets = 0x0000
Byte  4     protocol_type     = 0x00
Byte  5     data_size         = number of argument bytes used
Byte  6     command_class     = 0x0F (Extended Matrix)
Byte  7     command_id        = 0x02 (Set Effect)
Byte  8     args[0]           = 0x01 (VARSTORE)
Byte  9     args[1]           = led_id (device-specific)
Byte 10     args[2]           = 0x01 (EFFECT_STATIC)
Bytes 11-12 args[3-4]         = 0x00 (padding)
Byte 13     args[5]           = 0x01 (colour count)
Byte 14     args[6]           = R
Byte 15     args[7]           = G
Byte 16     args[8]           = B
Bytes 17-87 args[9-79]        = 0x00 (padding)
Byte 88     crc               = XOR of bytes [2..88)
Byte 89     reserved          = 0x00
```

**CRC calculation:**

```rust
let crc = buf[2..88].iter().fold(0u8, |acc, &b| acc ^ b);
```

### Kraken / Headset (37-byte report)

```
Byte  0     report_id         = 0x00
Bytes 1-3   led_address       (zone-specific, from Wireshark capture)
Byte  4     data_length
Bytes 5+    RGB / effect data
```

The exact `led_address` bytes per zone (left cup, right cup, headband) must be determined from the Wireshark capture.

---

## 5. Update `synaptix-protocol`

All shared types live in `crates/synaptix-protocol/src/lib.rs`. **Always update this crate first** before touching the daemon.

### Add the Product ID variant

```rust
pub enum RazerProductId {
    // ... existing variants ...

    // Your new device — add both wired and wireless if applicable
    KrakenV4ProWired,    // 0x0568
    KrakenV4ProWireless, // 0x0567
}
```

### Add the PID mapping

```rust
pub fn usb_pid(&self) -> u16 {
    match self {
        // ... existing mappings ...
        RazerProductId::KrakenV4ProWired    => 0x0568,
        RazerProductId::KrakenV4ProWireless => 0x0567,
    }
}
```

### Run the protocol tests

```bash
cargo test -p synaptix-protocol
```

---

## 6. Write the Payload Builder (TDD)

### For mouse-family devices

Add a test to `crates/synaptix-daemon/src/razer_protocol.rs` **before** implementing anything:

```rust
/// Exact payload for YourDevice with Red (0xFF, 0x00, 0x00).
/// CRC = XOR of all non-zero bytes in positions [2..88).
#[test]
fn test_static_color_payload_your_device() {
    let payload = build_static_color_payload(
        0x1F,  // transaction_id — from reference driver or Wireshark
        0x00,  // led_id        — from reference driver or Wireshark
        0xFF, 0x00, 0x00,
    );
    assert_eq!(payload[1],  0x1F, "transaction_id mismatch");
    assert_eq!(payload[9],  0x00, "led_id mismatch");
    assert_eq!(payload[14], 0xFF, "R mismatch");
    assert_eq!(payload[88], 0xFA, "CRC mismatch"); // calculate your expected CRC
}
```

Run it to confirm it fails first (Red phase), then verify it passes (Green phase):

```bash
cargo test -p synaptix-daemon razer_protocol
```

### For headset-family devices (new protocol)

Create a new file `crates/synaptix-daemon/src/razer_kraken_protocol.rs`:

```rust
pub const KRAKEN_REPORT_LEN: usize = 37;
pub const KRAKEN_W_VALUE: u16 = 0x0204;
pub const KRAKEN_W_INDEX: u16 = 0x0003;

/// TODO: fill in with LED addresses from Wireshark capture.
pub fn build_kraken_static_color_payload(r: u8, g: u8, b: u8) -> [u8; KRAKEN_REPORT_LEN] {
    let mut buf = [0u8; KRAKEN_REPORT_LEN];
    // ... populate from decoded payload ...
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kraken_static_color_red() {
        let payload = build_kraken_static_color_payload(0xFF, 0x00, 0x00);
        // assert known-good bytes from Wireshark capture
        assert_eq!(payload[0], 0x00);
        // ... add assertions for every byte that should be non-zero ...
    }
}
```

Then register the new module in `crates/synaptix-daemon/src/main.rs`:

```rust
mod razer_kraken_protocol;
```

---

## 7. Wire Up the Daemon

### For mouse-family devices — add `lighting_params`

In `crates/synaptix-daemon/src/device_manager.rs`, add your device to the `lighting_params` match:

```rust
fn lighting_params(product_id: &RazerProductId) -> (u8, u8) {
    use crate::razer_protocol::{LED_BACKLIGHT, LED_ZERO, TRANSACTION_ID_COBRA, TRANSACTION_ID_DA};
    match product_id {
        RazerProductId::CobraProWired | RazerProductId::CobraProWireless => {
            (TRANSACTION_ID_COBRA, LED_ZERO)
        }
        RazerProductId::DeathAdderV2Pro => (TRANSACTION_ID_DA, LED_BACKLIGHT),

        // ↓ Add your new device here
        RazerProductId::YourNewDevice => (YOUR_TRANSACTION_ID, YOUR_LED_ID),

        _ => (TRANSACTION_ID_DA, LED_BACKLIGHT), // safe default
    }
}
```

### For headset-family devices — extend `send_control_transfer`

Update `crates/synaptix-daemon/src/usb_backend.rs` to accept `wValue` and `wIndex` as parameters so both protocol families can share the same USB function:

```rust
pub fn send_control_transfer(
    product_id: u16,
    payload: &[u8],
    w_value: u16,
    w_index: u16,
) -> Result<(), rusb::Error> {
    // ... existing implementation, replace hardcoded 0x0300/0x0000 ...
}
```

Then call it from the Kraken branch of `set_lighting` with `KRAKEN_W_VALUE` and `KRAKEN_W_INDEX`.

### Seed the device in `main.rs` for manual testing

```rust
manager.add_device(
    "kraken-v4-pro".to_string(),
    RazerDevice {
        name: "Razer Kraken V4 Pro".to_string(),
        product_id: RazerProductId::KrakenV4ProWired,
        battery_state: BatteryState::Discharging(100),
    },
);
```

### Run all tests

```bash
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

All three must pass before opening a PR.

---

## 8. Update the README

Add your device to the **Supported Devices** table in `README.md`:

```markdown
| Razer Kraken V4 Pro | `0x0568` | `0x0567` | ✅ Tested | ❌ Not yet supported |
```

Update the status columns honestly:
- ✅ **Tested** — confirmed working on real hardware
- 🔄 **Simulated** — implemented but not yet verified on device
- ❌ **Not yet supported** — not implemented

---

## 9. Open a Pull Request

Include the following in your PR description:

- [ ] Device name, wired PID, and wireless PID (if applicable)
- [ ] Source of the PID (link to OpenRazer header, `lsusb` output, or Wireshark capture)
- [ ] Which protocol family was used and why
- [ ] Confirmation that `cargo test --workspace` passes
- [ ] Confirmation that `cargo clippy` and `cargo fmt --check` pass
- [ ] Whether the lighting was confirmed on **real hardware** or is based on protocol inference

---

## Reference: USB Control Transfer Parameters

| Parameter | Mouse family | Kraken headset family |
|---|---|---|
| `bmRequestType` | `0x21` | `0x21` |
| `bRequest` | `0x09` (HID SET_REPORT) | `0x09` |
| `wValue` | `0x0300` | `0x0204` |
| `wIndex` | `0x0000` | `0x0003` |
| Payload size | 90 bytes | 37 bytes |

## Reference: Battery Query Protocol

Battery level and charging status are queried using a **write + sleep + read** pattern (confirmed from `razercommon.c → razer_get_usb_response`):

### Step 1 — Send the query (SET_REPORT)

Same parameters as lighting (`bmRequestType=0x21`, `bRequest=0x09`), but with a battery-specific payload:

| Field | Battery Level | Charging Status |
|---|---|---|
| `[5]` data_size | `0x02` | `0x02` |
| `[6]` command_class | `0x07` (Battery) | `0x07` (Battery) |
| `[7]` command_id | `0x80` | `0x84` |
| `[88]` CRC | `0x02 ^ 0x07 ^ 0x80` | `0x02 ^ 0x07 ^ 0x84` |

### Step 2 — Sleep

Sleep **≥1 ms** to allow the device firmware to prepare the response. Newer wireless mice (Cobra Pro group) require this delay.

### Step 3 — Read the response (GET_REPORT)

```
bmRequestType = 0xA1  (DEVICE→HOST | CLASS | INTERFACE)
bRequest      = 0x01  (HID GET_REPORT)
wValue        = 0x0300
wIndex        = 0x00
```

Read 90 bytes. The answer is in **`response[9]`** (`arguments[1]`):
- Battery level: raw `0–255` → convert to percentage: `(raw as u16 * 100 / 255) as u8`
- Charging status: `0` = discharging, `1` = charging

### Step 4 — Determine BatteryState

```rust
let state = if is_charging && percent >= 100 {
    BatteryState::Full
} else if is_charging {
    BatteryState::Charging(percent)
} else {
    BatteryState::Discharging(percent)
};
```

### Adding battery support for a new device

1. Confirm the device's `transaction_id` (from `razermouse_driver.c` or Wireshark capture).
2. Add it to the `query_battery` call in `main.rs` — `query_battery(pid, transaction_id)`.
3. No changes to `razer_protocol.rs` are needed — `build_battery_query_payload(transaction_id)` is generic.



## Reference: Known `transaction_id` Values

| Device group | `transaction_id` | `led_id` |
|---|---|---|
| Cobra Pro (Wired/Wireless) | `0x1F` | `0x00` (ZERO_LED) |
| DeathAdder V2 Pro | `0x3F` | `0x05` (BACKLIGHT_LED) |
| Basilisk V3 Pro | `0x1F` | `0x00` |
| Most older wireless mice | `0x3F` | `0x05` |

When in doubt, try `0x1F` / `0x00` first for newer mice and `0x3F` / `0x05` for older ones — and always verify with a Wireshark capture.
