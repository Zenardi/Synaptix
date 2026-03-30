---
name: wireshark-usb-analysis
description: >
  Guide for analysing Wireshark .pcapng captures (taken on Windows with Razer Synapse running)
  to extract exact USB payloads needed to implement a new Razer device feature in Synaptix.
  Use this skill when asked to analyse a pcapng file, implement battery reporting, lighting,
  DPI, or haptics for a new device, or debug why a known USB payload is being rejected or timing out.
---

Analysing Wireshark `.pcapng` captures for Razer USB protocol extraction follows this structured process.

## When to Use This Skill

Use this workflow whenever you need to:

- Implement battery reporting for a **new device type** (headset, keyboard, controller)
- Implement a lighting or haptic effect whose USB payload is unknown
- Debug why a known-good payload is being rejected or timing out
- Confirm that the correct endpoint, interface, and report ID are being targeted

Do **not** guess payloads from the `_reference_openrazer/` Python source alone — the Python code often contains abstraction layers, fallback paths, and platform-specific branches that obscure the raw byte sequence. The pcapng is ground truth.

---

## Capture Checklist

### Hardware
- Use a **USB-A port** if possible — USB-C can leave composite devices in an inactive relay mode that suppresses push packets
- Razer Synapse must be **open and active** during capture

### Capture steps
1. Install Wireshark on Windows with **USBPcap** selected
2. Start capture on the USBPcap interface **before** performing the action to observe
3. Trigger the feature: battery → open Synapse and wait ~5s; lighting → change colour; DPI → change value; haptics → trigger notification
4. Capture for **60 seconds** minimum
5. Save as `<device>_<feature>.pcapng` and copy to `wireshark/` in this repo

---

## Tooling Setup

```bash
sudo apt install tshark
export CAP=wireshark/your_capture.pcapng
```

---

## Phase 1 — Device Identification

### Step 1 — List all Razer USB devices in the capture

```bash
tshark -r $CAP -T fields \
  -e usb.bus_id -e usb.device_address \
  -e usb.idVendor -e usb.idProduct \
  -Y "usb.idVendor == 0x1532" \
  | sort -u
```

Match the PID against `crates/synaptix-protocol/src/registry.rs`.

### Step 2 — Map endpoints per device address

```bash
export DEV=3   # replace with your device address
tshark -r $CAP -T fields \
  -e usb.endpoint_address \
  -e usb.transfer_type \
  -e usb.data_len \
  -Y "usb.device_address == $DEV" \
  | sort | uniq -c | sort -rn | head -20
```

**Transfer types:** `0x02` = Control (commands), `0x03` = Interrupt (push data), `0x01` = Isochronous (audio, ignore).

### Step 3 — Identify control transfers (commands)

```bash
tshark -r $CAP -T fields \
  -e frame.number -e frame.time_relative \
  -e usb.setup.bmRequestType -e usb.setup.bRequest \
  -e usb.setup.wValue -e usb.setup.wIndex \
  -e usb.data_len \
  -Y "usb.device_address == $DEV && usb.setup.bRequest == 0x09"
```

### Step 4 — Identify interrupt-IN push packets (battery, input)

```bash
tshark -r $CAP -T fields \
  -e frame.number -e frame.time_relative \
  -e usb.endpoint_address -e usb.data_len \
  -Y "usb.device_address == $DEV && usb.transfer_type == 0x03 && usb.endpoint_address.direction == 1"
```

> Direction 1 = IN (device→host). Direction 0 = OUT (host→device).

---

## Phase 2 — Feature-Specific Analysis

### Battery Reporting

Battery data arrives as an **Interrupt-IN push packet**. The device either pushes autonomously after USB enumeration or in response to an OUTPUT trigger.

**Find battery push packets:**
```bash
tshark -r $CAP -T fields \
  -e frame.number -e frame.time_relative \
  -e usb.endpoint_address -e usb.capdata \
  -Y "usb.device_address == $DEV && usb.transfer_type == 0x03 && usb.endpoint_address.direction == 1" \
  | head -30
```

**Typical battery packet patterns:**

| Format | Devices | Notes |
|--------|---------|-------|
| `02 02 <pct> ...` | Kraken V4 Pro headset | `byte[2]` = direct 0–100% |
| `00 <state> <pct_raw> ...` | Cobra Pro mouse | `byte[1]` = charging flag, `byte[2]` = raw/255 |

**Find the trigger** (what Synapse sends just before the push):
```bash
tshark -r $CAP -T fields \
  -e frame.number -e frame.time_relative \
  -e usb.setup.wIndex -e usb.capdata \
  -Y "usb.device_address == $DEV && usb.setup.bRequest == 0x09"
```

**Decode percentage:** Compare `byte[N]` against what Synapse displays.
- Direct: `byte[N]` = 0–100 (e.g. `0x60` = 96 → Synapse shows 96%)
- Scaled: `byte[N]` = 0–255 → `pct = raw * 100 / 255`

### Lighting (RGB)

Razer lighting commands are **90-byte control transfers** (SET_REPORT) on Interface 0 (or higher for composite devices).

**Extract lighting commands:**
```bash
tshark -r $CAP -T fields \
  -e frame.number -e usb.setup.wIndex -e usb.capdata \
  -Y "usb.device_address == $DEV && usb.setup.bRequest == 0x09" \
  | head -30
```

**90-byte Razer packet structure:**
```
Byte 0     : 0x00 (status)
Byte 1     : Transaction ID (device-specific, e.g. 0x1F for Cobra Pro)
Bytes 2–3  : 0x00 0x00
Byte 4     : 0x00 (protocol)
Byte 5     : Data length
Byte 6     : Command class (0x0F = lighting)
Byte 7     : Command ID   (0x02 = static colour)
Bytes 8+   : Payload (R, G, B for static)
Byte 89    : Checksum (XOR of bytes 2–88)
```

### DPI Control

DPI commands are 90-byte control transfers with command class `0x04`. DPI is typically a little-endian 16-bit value in bytes 9–10 (X) and 11–12 (Y), divided by a device-specific step.

### Haptics / Rumble

Haptic commands are **raw HID OUTPUT reports** (not 90-byte Razer protocol) on the haptic interface. Sort by interface (`wIndex`) to isolate them from lighting commands.

---

## Linux vs Windows USB Layout

**Critical:** Windows splits composite devices into multiple logical USB devices. Linux presents them as a single device with multiple interfaces.

**Windows pcapng shows:**
```
Bus 1, Device 3 = HID haptics+battery  (Interface 0 on Windows)
```

**Linux rusb sees:**
```
Bus 3, Device 10 (PID 0x0568)
  Interface 4: HID haptics+battery (ep 0x84, NO kernel driver)
```

**Translation rule:** Find the endpoint address from the pcapng, then run on Linux:
```bash
lsusb -v -d 1532:<pid> | grep -A3 "bInterfaceNumber\|bEndpointAddress"
```
The interface that owns that endpoint is what you pass to `claim_interface()` and `wIndex`.

---

## Critical Protocol Rules (Hard-Won)

| Rule | Why |
|------|-----|
| **Never call `clear_halt()` on an interrupt-IN endpoint before reading** | Resets DATA toggle → device sends DATA1 but host expects DATA0 → alternating FAIL/SUCCESS |
| **Use ≥ 3000 ms read timeout on first contact** | 500 ms causes spurious failures; device may take up to ~3s to respond on cold start |
| **Send OUTPUT trigger before reading, not after** | Device queues response immediately on trigger; reading first can miss it |
| **USB reset fallback for extended USB-C idle** | Kraken V4 Pro enters "inactive relay mode"; `handle.reset()` forces re-enumeration |
| **Interface 0 is NOT always the control interface** | Composite devices use higher-numbered interfaces; verify with `lsusb -v` |
| **90-byte checksum must be correct** | Wrong checksum → device silently accepts USB transfer but does nothing |
| **Always `claim_interface()` before read/write** | Required even when no kernel driver is bound; otherwise `LIBUSB_ERROR_ACCESS` |

---

## Confirmed Protocol Registry

### Razer Cobra Pro — Battery (PIDs: 0x00AF, 0x00B0)

- Interface: 0, Transaction ID: `0x1F`, wait: 31 ms
- Command: class `0x07`, cmd_id `0x80` (get battery)
- Response: `byte[8]` = charging (0x01/0x00), `byte[9]` = raw (0–255), `pct = raw * 100 / 255`

### Razer Cobra Pro — Lighting (PIDs: 0x00AF, 0x00B0)

- Interface: 0, Command class: `0x0F`, Command ID (static): `0x02`
- Payload: `00 1F 00 00 00 05 0F 02 [led_id] [0xFF] [R] [G] [B] ...`
- `led_id`: `0x01` = scroll wheel, `0x07` = logo, `0x0E` = underglow

### Razer Kraken V4 Pro — Battery (PID: 0x0568)

- Linux Interface: 4 (no kernel driver), Interrupt-IN endpoint: `0x84`, packet: 64 bytes
- Packet: `byte[0]=0x02, byte[1]=0x02, byte[2]=<pct direct 0–100>`
- Trigger: `ctrl_transfer(bmReqType=0x21, bReq=0x09, wVal=0x0202, wIdx=4, data=64bytes)`
- Sequence: claim → trigger → `read_interrupt(ep=0x84, timeout=3000ms)` → validate → release
- Fallback: if 3 attempts fail → `handle.reset()` → wait 2s → retry
- ❌ Never `clear_halt(0x84)`, never timeout < 1000 ms

---

## New Device Worksheet Template

When starting on a new device, copy this template and commit it to `wireshark/` alongside the `.pcapng` file.

```markdown
# Protocol Analysis: [Device Name] — [Feature]

**Date:** YYYY-MM-DD
**Capture file:** `wireshark/<filename>.pcapng`
**Synaptix PID:** `0x????`
**Windows USB address (in capture):** Bus X, Device Y
**Linux interface (verified with lsusb -v):** Interface N

## Endpoint Map
| Endpoint | Direction | Transfer Type | Packet Size | Presumed Purpose |
|----------|-----------|--------------|-------------|------------------|
| 0x?? | IN/OUT | Control/Interrupt | N bytes | |

## Feature: [Battery / Lighting / DPI / Haptics]

### Trigger
- bmRequestType: `0x??`  bRequest: `0x??`  wValue: `0x????`  wIndex: `N`
- Payload (first 16 bytes): `XX XX XX XX XX XX XX XX XX XX XX XX XX XX XX XX`

### Response packet
- Endpoint: `0x??`, Type: Interrupt IN / Control IN, Length: N bytes
- Raw bytes (example): `XX XX XX XX ...`

### Payload decode
| Byte | Value | Meaning |
|------|-------|---------|
| 0 | `0x??` | Report ID |
| 2 | `0x??` | [field] |

### Value formula
- Direct: `value = byte[N]`
- Scaled: `value = byte[N] * 100 / 255`
- 16-bit: `value = (byte[N] << 8 | byte[N+1]) / STEP`

### Notes
[Timing requirements, driver conflicts, gotchas]
```
