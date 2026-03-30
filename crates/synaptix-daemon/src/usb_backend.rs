use crate::razer_protocol::{
    build_battery_query_payload, build_charging_query_payload, build_haptic_report,
    parse_headset_push_packet, validate_response, CMD_CLASS_BATTERY, CMD_ID_BATTERY_LEVEL,
    CMD_ID_CHARGING_STATUS, HAPTIC_REPORT_LEN, RAZER_VID, REPORT_LEN,
};
use rusb::{Context, DeviceHandle, UsbContext};
use synaptix_protocol::{registry::get_device_profile, BatteryState, ConnectionType};

/// PID of the Cobra Pro wired interface (USB cable). Used to detect whether the
/// cable is plugged in even when the active connection is via the dongle.
pub const COBRA_PRO_WIRED_PID: u16 = 0x00AF;

/// PID of the BlackWidow V3 Mini HyperSpeed wired interface. Used to detect
/// whether the USB cable is plugged in when the keyboard is in wireless mode.
pub const BLACKWIDOW_V3_MINI_WIRED_PID: u16 = 0x0258;

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
/// - `Dongle`: gaming wirelessly; charging is detected by checking whether
///   `wired_companion_pid` is also present on the USB bus (cable plugged in).
/// - `Bluetooth`: falls back to the firmware's charging-status response byte.
///
/// `wired_companion_pid` is only used for `Dongle` connections. Pass the USB
/// PID of the same device's wired interface (e.g. `COBRA_PRO_WIRED_PID` for the
/// Cobra Pro, `BLACKWIDOW_V3_MINI_WIRED_PID` for the BW V3 Mini). Pass `None`
/// to skip the wired-fallback and rely solely on the firmware charging byte.
pub fn query_battery(
    product_id: u16,
    transaction_id: u8,
    wait_us: u64,
    connection_type: &ConnectionType,
    wired_companion_pid: Option<u16>,
) -> rusb::Result<BatteryState> {
    println!(
        "[Battery] Querying battery for PID 0x{product_id:04x}, txn_id=0x{transaction_id:02x}, wait={wait_us}µs, conn={connection_type:?}"
    );

    let (handle, iface_u8) = open_razer_device(product_id)?;
    let iface = iface_u8 as u16;
    let timeout = std::time::Duration::from_millis(500);
    let sleep = std::time::Duration::from_micros(wait_us);

    // ── Battery level ─────────────────────────────────────────────────────────
    // Dongle+cable: when both the dongle PID and wired PID are present the
    // dongle firmware may return a valid SUCCESSFUL response but with 0 for the
    // level (device switches power source to USB and stops reporting internal
    // cell level via the wireless HID channel). Fall back to the wired interface
    // which keeps tracking the battery correctly.
    let percent = {
        let dongle_pct = query_level(&handle, iface, transaction_id, &sleep, timeout)?;

        if dongle_pct == 0 {
            if let (ConnectionType::Dongle, Some(wired_pid)) =
                (connection_type, wired_companion_pid)
            {
                if detect_connected_pid(&[wired_pid]).is_some() {
                    println!(
                        "[Battery] Dongle returned 0%; trying wired interface (0x{wired_pid:04x}) for level…"
                    );
                    match open_razer_device(wired_pid).and_then(|(h, wi)| {
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
    // Dongle: wireless gaming; charging detected via wired_companion_pid scan.
    // Bluetooth / firmware fallback: use the charging-status HID command.
    let is_charging = match connection_type {
        ConnectionType::Wired => {
            println!("[Battery] Wired connection — forcing is_charging=true");
            true
        }
        ConnectionType::Dongle => {
            let cable_present = wired_companion_pid
                .map(|wired_pid| detect_connected_pid(&[wired_pid]).is_some())
                .unwrap_or(false);
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

/// Attempts to query the Kraken V4 Pro battery via a single trigger+read cycle.
/// Returns `Some(pct)` on the first successful attempt, `None` if all 3 attempts fail.
fn try_headset_battery_query(product_id: u16) -> Option<u8> {
    let (handle, _iface_u8) = open_razer_device(product_id)
        .inspect_err(|e| log::warn!("[HeadsetBatt] open_razer_device failed: {e:?}"))
        .ok()?;

    let ctrl_timeout = std::time::Duration::from_millis(500);
    let read_timeout = std::time::Duration::from_millis(500);

    for attempt in 1..=3usize {
        let trigger = build_haptic_report(0);
        match handle.write_control(0x21, 0x09, 0x0202, 0x0004, &trigger, ctrl_timeout) {
            Ok(_) => log::info!("[HeadsetBatt] Sent OUTPUT trigger (attempt {attempt})"),
            Err(e) => {
                log::warn!("[HeadsetBatt] OUTPUT trigger failed (attempt {attempt}): {e:?}");
                if attempt < 3 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                continue;
            }
        }

        // Device responds within ~10 ms on a freshly-enumerated USB link.
        let mut resp = [0u8; HAPTIC_REPORT_LEN];
        match handle.read_interrupt(0x84, &mut resp, read_timeout) {
            Ok(_) => {
                log::info!(
                    "[HeadsetBatt] Response (attempt {attempt}): {:02x} {:02x} {:02x} …",
                    resp[0],
                    resp[1],
                    resp[2]
                );
                if let Some(pct) = parse_headset_push_packet(&resp) {
                    log::info!("[HeadsetBatt] battery={pct}% (byte[2]=0x{:02x})", resp[2]);
                    return Some(pct);
                }
                log::warn!(
                    "[HeadsetBatt] Not a battery packet (attempt {attempt}): byte[1]=0x{:02x} byte[2]={}",
                    resp[1],
                    resp[2]
                );
            }
            Err(e) => {
                log::warn!("[HeadsetBatt] read_interrupt failed (attempt {attempt}): {e:?}");
            }
        }

        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    None
}

/// Reads the Kraken V4 Pro headset battery level from the HID interface.
///
/// **Protocol (Wireshark-verified, `battery_synapse.pcapng`):**
/// Battery packets are 64 bytes starting with `02 02 <pct> ...` on ep=0x84 of Interface 4.
/// `byte[2]` is the direct percentage (0–100, e.g. 0x60 = 96%).
///
/// **Trigger mechanism:** The device only pushes battery data in response to a HID SET_REPORT
/// (OUTPUT) on Interface 4. If all attempts fail (device in "inactive relay mode"), a USB
/// device reset is issued to force re-enumeration and the query is retried.
pub fn poll_headset_battery(product_id: u16) -> Option<u8> {
    // First attempt: normal query.
    if let Some(pct) = try_headset_battery_query(product_id) {
        return Some(pct);
    }

    // All attempts failed. The Kraken V4 Pro hub enters an "inactive relay mode"
    // when it has been idle for a while (observed when switching USB ports or after
    // extended uptime). A USB bus reset forces the hub to re-enumerate, which puts
    // it back into active mode. We then retry once.
    log::warn!(
        "[HeadsetBatt] All attempts failed for PID={product_id:#06x} — issuing USB reset to reactivate hub"
    );

    if let Ok((handle, _)) = open_razer_device(product_id) {
        if let Err(e) = handle.reset() {
            log::warn!("[HeadsetBatt] USB reset failed: {e:?}");
        } else {
            log::info!("[HeadsetBatt] USB reset OK — waiting for re-enumeration");
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Some(pct) = try_headset_battery_query(product_id) {
                log::info!("[HeadsetBatt] Post-reset query succeeded: {pct}%");
                return Some(pct);
            }
        }
    }

    log::warn!(
        "[HeadsetBatt] Post-reset query also failed for PID={product_id:#06x} — battery stays Unknown"
    );
    None
}
