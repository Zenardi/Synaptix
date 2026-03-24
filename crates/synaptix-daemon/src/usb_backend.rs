use crate::razer_protocol::{
    build_battery_query_payload, build_charging_query_payload, build_headset_battery_query,
    validate_response, CMD_CLASS_BATTERY, CMD_ID_BATTERY_LEVEL, CMD_ID_CHARGING_STATUS,
    HAPTIC_REPORT_LEN, RAZER_VID, REPORT_LEN,
};
use rusb::{Context, DeviceHandle, UsbContext};
use synaptix_protocol::{registry::get_device_profile, BatteryState, ConnectionType};

/// PID of the Cobra Pro wired interface (USB cable). Used to detect whether the
/// cable is plugged in even when the active connection is via the dongle.
pub const COBRA_PRO_WIRED_PID: u16 = 0x00AF;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Opens the first Razer device matching `product_id` and returns its handle
/// (with kernel driver detached and the correct interface claimed) together with
/// the `control_interface` index read from the device registry.
///
/// The interface index is also used as `wIndex` in subsequent HID control
/// transfers — callers must forward it to `write_control` / `read_control`.
fn open_razer_device(product_id: u16) -> rusb::Result<(DeviceHandle<Context>, u8)> {
    // Look up the control interface from the registry; fall back to 0 for
    // unknown devices so that ad-hoc calls (e.g. battery queries on a PID not
    // yet registered) still work.
    let control_interface = get_device_profile(product_id)
        .map(|p| p.control_interface)
        .unwrap_or(0);

    println!("[USB] Searching for Razer device with Product ID: 0x{product_id:04x}");
    let ctx = Context::new()?;
    let devices = ctx.devices()?;
    println!("[USB] Scanning {} USB device(s) …", devices.len());

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[USB] Could not read descriptor for a device: {e:?}");
                continue;
            }
        };

        if desc.vendor_id() != RAZER_VID || desc.product_id() != product_id {
            continue;
        }

        println!(
            "[USB] Found Razer device {:04x}:{:04x} — opening …",
            desc.vendor_id(),
            desc.product_id()
        );

        let handle = device.open().inspect_err(|e| {
            eprintln!("[USB] Failed to open device: {e:?}");
        })?;

        println!("[USB] Device opened successfully.");

        if let Err(e) = handle.set_auto_detach_kernel_driver(true) {
            eprintln!(
                "[USB] set_auto_detach_kernel_driver failed (non-fatal on some kernels): {e:?}"
            );
        }

        handle.claim_interface(control_interface).inspect_err(|e| {
            eprintln!("[USB] claim_interface({control_interface}) failed: {e:?}");
        })?;

        println!("[USB] Interface {control_interface} claimed.");
        return Ok((handle, control_interface));
    }

    eprintln!("[USB] No Razer device with PID 0x{product_id:04x} found.");
    Err(rusb::Error::NoDevice)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Scans the USB bus for the first PID in `candidates` (tried in order) that
/// is currently attached with Razer VID `0x1532`.
///
/// Returns the matching PID, or `None` if no candidate is present.
/// Used at startup and in the polling loop to detect connection-type changes
/// (wired vs HyperSpeed dongle).
pub fn detect_connected_pid(candidates: &[u16]) -> Option<u16> {
    let ctx = Context::new().ok()?;
    let devices = ctx.devices().ok()?;
    for pid in candidates {
        for device in devices.iter() {
            if let Ok(desc) = device.device_descriptor() {
                if desc.vendor_id() == RAZER_VID && desc.product_id() == *pid {
                    return Some(*pid);
                }
            }
        }
    }
    None
}

/// Sends a 90-byte HID SET_REPORT control transfer to a Razer device.
///
/// ```text
/// bmRequestType = 0x21  (HOST→DEVICE | CLASS | INTERFACE)
/// bRequest      = 0x09  (HID SET_REPORT)
/// wValue        = 0x0300
/// wIndex        = <control_interface from registry>
/// data          = 90-byte report
/// ```
pub fn send_control_transfer(product_id: u16, payload: &[u8; REPORT_LEN]) -> rusb::Result<()> {
    let (handle, iface) = open_razer_device(product_id)?;
    let timeout = std::time::Duration::from_millis(500);

    let n = handle
        .write_control(0x21, 0x09, 0x0300, iface as u16, payload, timeout)
        .inspect_err(|e| eprintln!("[USB] write_control failed: {e:?}"))?;

    println!("[USB] write_control returned {n} bytes (expected {REPORT_LEN}).");
    if n != REPORT_LEN {
        eprintln!("[USB] Short write — expected {REPORT_LEN}, got {n}.");
        return Err(rusb::Error::Io);
    }

    println!("[USB] Control transfer complete.");
    Ok(())
}

/// Sends a 64-byte proprietary HID report to the Kraken V4 Pro OLED Hub.
///
/// This is a completely separate protocol path from the legacy 90-byte Razer
/// HID reports. Wireshark-verified Setup Packet: `21 09 00 03 04 00 40 00`.
///
/// ```text
/// bmRequestType = 0x21   (HOST→DEVICE | CLASS | INTERFACE)
/// bRequest      = 0x09   (HID SET_REPORT)
/// wValue        = 0x0202 (Output Report 2)
/// wIndex        = 0x0004 (Interface 4, from registry)
/// wLength       = 64
/// timeout       = 1 000 ms
/// ```
///
/// Returns `Ok(())` on success. `rusb::Error::Timeout` means the firmware did
/// not ACK within 1 s — likely a wrong interface or incorrect wValue.
/// `rusb::Error::Pipe` (stall) means the firmware rejected the request type.
pub fn send_haptic_report(product_id: u16, payload: &[u8; HAPTIC_REPORT_LEN]) -> rusb::Result<()> {
    let (handle, iface) = open_razer_device(product_id)?;
    let timeout = std::time::Duration::from_millis(1_000);

    let n = match handle.write_control(0x21, 0x09, 0x0202, iface as u16, payload, timeout) {
        Ok(n) => n,
        Err(rusb::Error::Timeout) => {
            eprintln!(
                "[USB] Haptic SET_REPORT timed out for PID={product_id:#06x} — \
                 verify wIndex={iface} and wValue=0x0202"
            );
            return Err(rusb::Error::Timeout);
        }
        Err(rusb::Error::Pipe) => {
            eprintln!(
                "[USB] Haptic SET_REPORT stalled (EPIPE) for PID={product_id:#06x} — \
                 firmware rejected the request type or report ID"
            );
            return Err(rusb::Error::Pipe);
        }
        Err(e) => {
            eprintln!("[USB] Haptic write_control failed: {e:?}");
            return Err(e);
        }
    };

    println!("[USB] Haptic write_control returned {n} bytes (expected {HAPTIC_REPORT_LEN}).");
    if n != HAPTIC_REPORT_LEN {
        eprintln!("[USB] Short write — expected {HAPTIC_REPORT_LEN}, got {n}.");
        return Err(rusb::Error::Io);
    }

    println!("[USB] Haptic control transfer complete.");
    Ok(())
}

/// Queries battery level (0–100%) from an already-open device handle.
///
/// Retries up to 3 times on STATUS_BUSY (0x01). Returns `Err` if BUSY after
/// all retries or if the firmware returns a hard-failure status.
fn query_level(
    handle: &DeviceHandle<Context>,
    iface: u16,
    transaction_id: u8,
    sleep: &std::time::Duration,
    timeout: std::time::Duration,
) -> rusb::Result<u8> {
    let level_query = build_battery_query_payload(transaction_id);
    for attempt in 1..=3u8 {
        handle
            .write_control(0x21, 0x09, 0x0300, iface, &level_query, timeout)
            .inspect_err(|e| eprintln!("[Battery] SET_REPORT (level) failed: {e:?}"))?;
        std::thread::sleep(*sleep);
        let mut resp = [0u8; REPORT_LEN];
        handle
            .read_control(0xA1, 0x01, 0x0300, iface, &mut resp, timeout)
            .inspect_err(|e| eprintln!("[Battery] GET_REPORT (level) failed: {e:?}"))?;
        match validate_response(&resp, CMD_CLASS_BATTERY, CMD_ID_BATTERY_LEVEL) {
            Ok(()) => {
                let raw = resp[9];
                let pct = ((raw as u16 * 100) / 255) as u8;
                println!("[Battery] Level: raw={raw}/255 → {pct}% (attempt {attempt})");
                return Ok(pct);
            }
            Err(false) => {
                println!("[Battery] BUSY on level query (attempt {attempt}), retrying…");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(true) => {
                eprintln!("[Battery] Level query failed (bad response status).");
                return Err(rusb::Error::Io);
            }
        }
    }
    eprintln!("[Battery] Level query BUSY after 3 attempts.");
    Err(rusb::Error::Io)
}

/// Queries the physical device for its current battery level and charging status.
///
/// Protocol (confirmed from `razercommon.c → razer_get_usb_response`):
/// 1. Send query via SET_REPORT (`bmRequestType=0x21`, `bRequest=0x09`)
/// 2. Sleep `wait_us` microseconds — firmware needs time to prepare the response.
///    Cobra Pro / new receivers require ≥31 ms (`WAIT_NEW_RECEIVER_US`).
/// 3. Read response via GET_REPORT (`bmRequestType=0xA1`, `bRequest=0x01`)
/// 4. `response[9]` (`arguments[1]`) holds the value:
///    - battery level: raw 0–255 → scale to 0–100%
///    - charging:      0 = discharging, 1 = charging
///
/// `connection_type` is used to infer charging state more reliably:
/// - `Wired`: the USB cable *is* the power source — always charging.
/// - `Dongle`: gaming wirelessly; charging is detected by checking whether the
///   wired interface (0x00AF) is also present on the USB bus (cable plugged in).
/// - `Bluetooth`: falls back to the firmware's charging-status response byte.
pub fn query_battery(
    product_id: u16,
    transaction_id: u8,
    wait_us: u64,
    connection_type: &ConnectionType,
) -> rusb::Result<BatteryState> {
    println!(
        "[Battery] Querying battery for PID 0x{product_id:04x}, txn_id=0x{transaction_id:02x}, wait={wait_us}µs, conn={connection_type:?}"
    );

    let (handle, iface_u8) = open_razer_device(product_id)?;
    let iface = iface_u8 as u16;
    let timeout = std::time::Duration::from_millis(500);
    let sleep = std::time::Duration::from_micros(wait_us);

    // ── Battery level ─────────────────────────────────────────────────────────
    // Dongle+cable: when both 0x00B0 (dongle) and 0x00AF (wired) are present
    // the dongle firmware may return a valid SUCCESSFUL response but with 0 for
    // the level (the mouse switches power source to USB and stops reporting
    // internal cell level via the wireless HID channel). Fall back to the wired
    // interface (0x00AF) which keeps tracking the battery correctly.
    let percent = {
        let dongle_pct = query_level(&handle, iface, transaction_id, &sleep, timeout)?;

        if dongle_pct == 0 {
            if let ConnectionType::Dongle = connection_type {
                if detect_connected_pid(&[COBRA_PRO_WIRED_PID]).is_some() {
                    println!(
                        "[Battery] Dongle returned 0%; trying wired interface (0x{COBRA_PRO_WIRED_PID:04x}) for level…"
                    );
                    match open_razer_device(COBRA_PRO_WIRED_PID).and_then(|(h, wi)| {
                        query_level(&h, wi as u16, transaction_id, &sleep, timeout)
                    }) {
                        Ok(wired_pct) if wired_pct > 0 => {
                            println!(
                                "[Battery] Wired interface returned {wired_pct}% — using this."
                            );
                            wired_pct
                        }
                        _ => dongle_pct,
                    }
                } else {
                    dongle_pct
                }
            } else {
                dongle_pct
            }
        } else {
            dongle_pct
        }
    };

    // ── Charging status ───────────────────────────────────────────────────────
    //
    // Wired: USB cable supplies power directly — always charging by definition.
    // Dongle: wireless gaming; charging happens on the *cable* interface
    //         (0x00AF). We detect it via a cheap PID scan — no device open.
    // Bluetooth / firmware fallback: use the charging-status HID command.
    let is_charging = match connection_type {
        ConnectionType::Wired => {
            println!("[Battery] Wired connection — forcing is_charging=true");
            true
        }
        ConnectionType::Dongle => {
            let cable_present = detect_connected_pid(&[COBRA_PRO_WIRED_PID]).is_some();
            println!("[Battery] Dongle connection, cable present: {cable_present}");
            if cable_present {
                true
            } else {
                query_charging_status(&handle, iface, transaction_id, &sleep, timeout)?
            }
        }
        ConnectionType::Bluetooth => {
            query_charging_status(&handle, iface, transaction_id, &sleep, timeout)?
        }
    };

    println!("[Battery] Charging: {is_charging}");

    let state = if is_charging && percent >= 100 {
        BatteryState::Full
    } else if is_charging {
        BatteryState::Charging(percent)
    } else {
        BatteryState::Discharging(percent)
    };

    println!("[Battery] Resolved state: {state:?}");
    Ok(state)
}

/// Sends a charging-status HID query and returns whether the device is charging.
fn query_charging_status(
    handle: &DeviceHandle<Context>,
    iface: u16,
    transaction_id: u8,
    sleep: &std::time::Duration,
    timeout: std::time::Duration,
) -> rusb::Result<bool> {
    let charging_query = build_charging_query_payload(transaction_id);

    for attempt in 1..=3u8 {
        handle
            .write_control(0x21, 0x09, 0x0300, iface, &charging_query, timeout)
            .inspect_err(|e| eprintln!("[Battery] SET_REPORT (charging) failed: {e:?}"))?;

        std::thread::sleep(*sleep);

        let mut charging_response = [0u8; REPORT_LEN];
        handle
            .read_control(0xA1, 0x01, 0x0300, iface, &mut charging_response, timeout)
            .inspect_err(|e| eprintln!("[Battery] GET_REPORT (charging) failed: {e:?}"))?;

        match validate_response(
            &charging_response,
            CMD_CLASS_BATTERY,
            CMD_ID_CHARGING_STATUS,
        ) {
            Ok(()) => {
                let is_charging = charging_response[9] != 0;
                println!("[Battery] Charging status: {is_charging} (attempt {attempt})");
                return Ok(is_charging);
            }
            Err(false) => {
                println!("[Battery] BUSY on charging query (attempt {attempt}), retrying…");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(true) => {
                eprintln!("[Battery] Charging query failed (bad response status).");
                return Err(rusb::Error::Io);
            }
        }
    }

    eprintln!("[Battery] Charging query BUSY after 3 attempts.");
    Err(rusb::Error::Io)
}

/// Attempts to read the Kraken V4 Pro headset battery level using the 64-byte
/// HID protocol (wValue=0x0202, wIndex=0x0004 on Interface 4).
///
/// Returns `Some(percent)` on success or `None` if the device does not respond
/// as expected. Callers should record `BatteryState::Unknown` on `None`.
pub fn query_headset_battery(product_id: u16) -> Option<u8> {
    let (handle, iface_u8) = open_razer_device(product_id)
        .inspect_err(|e| eprintln!("[HeadsetBatt] Failed to open device: {e:?}"))
        .ok()?;

    let iface = iface_u8 as u16;
    let timeout = std::time::Duration::from_millis(500);
    let query = build_headset_battery_query();

    // wValue=0x0202 — Report Type 2, Report ID 2 (same as haptic commands)
    handle
        .write_control(0x21, 0x09, 0x0202, iface, &query, timeout)
        .inspect_err(|e| eprintln!("[HeadsetBatt] SET_REPORT failed: {e:?}"))
        .ok()?;

    std::thread::sleep(std::time::Duration::from_millis(10));

    let mut resp = [0u8; HAPTIC_REPORT_LEN];
    handle
        .read_control(0xA1, 0x01, 0x0202, iface, &mut resp, timeout)
        .inspect_err(|e| eprintln!("[HeadsetBatt] GET_REPORT failed: {e:?}"))
        .ok()?;

    // byte[1] = status: 0x02 = success, anything else = failure/busy
    if resp[1] != 0x02 {
        eprintln!(
            "[HeadsetBatt] Unexpected response status: 0x{:02x} — battery query unsupported?",
            resp[1]
        );
        return None;
    }

    // Validate that the response echoes the expected command class and ID.
    // If the device doesn't understand the command it often echoes our payload
    // or returns zeroes — either way the cmd bytes won't match.
    if resp[6] != CMD_CLASS_BATTERY || resp[7] != CMD_ID_BATTERY_LEVEL {
        eprintln!(
            "[HeadsetBatt] Response cmd mismatch: class=0x{:02x} id=0x{:02x} (expected 0x07/0x80)",
            resp[6], resp[7]
        );
        return None;
    }

    let raw = resp[9];
    // 0x00 = device echoed zeros (command not executed).
    // 0xFF = device filled unset byte with all-ones (common "not supported" pattern).
    // Both map to an implausible percentage and are rejected.
    if raw == 0 || raw == 0xFF {
        eprintln!(
            "[HeadsetBatt] Response byte[9]=0x{raw:02x} — treating as unsupported (raw 0/255 is garbage)."
        );
        return None;
    }

    let pct = ((raw as u16 * 100) / 255) as u8;
    println!("[HeadsetBatt] Level: raw={raw}/255 → {pct}%");
    Some(pct)
}
