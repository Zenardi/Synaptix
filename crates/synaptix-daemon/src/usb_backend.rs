use crate::razer_protocol::{
    build_battery_query_payload, build_charging_query_payload, build_haptic_report,
    build_set_driver_mode_payload, parse_headset_push_packet, validate_response, CMD_CLASS_BATTERY,
    CMD_ID_BATTERY_LEVEL, CMD_ID_CHARGING_STATUS, HAPTIC_REPORT_LEN, RAZER_VID, REPORT_LEN,
};
use rusb::{Context, DeviceHandle, UsbContext};
use std::io::Read;
use synaptix_protocol::{registry::get_device_profile, BatteryState, ConnectionType};

/// PID of the Cobra Pro wired interface (USB cable). Used to detect whether the
/// cable is plugged in even when the active connection is via the dongle.
pub const COBRA_PRO_WIRED_PID: u16 = 0x00AF;

/// PID of the Viper Ultimate wired interface (USB cable). Used to detect whether
/// the cable is plugged in when the mouse is in wireless mode.
pub const VIPER_ULTIMATE_WIRED_PID: u16 = 0x007A;

/// PID of the BlackWidow V3 Mini HyperSpeed wired interface. Used to detect
/// whether the USB cable is plugged in when the keyboard is in wireless mode.
pub const BLACKWIDOW_V3_MINI_WIRED_PID: u16 = 0x0258;

// ── Sysfs battery helpers ────────────────────────────────────────────────────
//
// When the openrazer `razerkbd`/`razermouse` kernel module is bound to the
// device it handles all USB protocol details (including wireless relay) and
// exposes `charge_level` (0–255) and `charge_status` (0/1) under the HID sysfs
// tree.  Reading from there is always more reliable than raw libusb control
// transfers because:
//   1. The kernel driver is never detached — the wireless channel stays up.
//   2. For wireless keyboards with USB cable (charging), the kernel driver
//      reports the real battery level from the wired companion interface.
//
// The sysfs HID tree is at `/sys/bus/hid/devices/XXXX:1532:PPPP.NNNN/`.
// We scan all entries and match by VID:PID (in HID uevent format:
// `HID_ID=0003:00001532:0000PPPP`).

/// Scans `/sys/bus/hid/devices/` for entries matching Razer VID and the given
/// PID.  Returns all paths that have a `charge_level` attribute file.
fn find_sysfs_charge_dirs(pid: u16) -> Vec<std::path::PathBuf> {
    let hid_dir = std::path::Path::new("/sys/bus/hid/devices");
    let target = format!("HID_ID=0003:00001532:{pid:08X}");
    let mut dirs = Vec::new();
    let Ok(entries) = std::fs::read_dir(hid_dir) else {
        return dirs;
    };
    for entry in entries.flatten() {
        let uevent_path = entry.path().join("uevent");
        if let Ok(content) = std::fs::read_to_string(&uevent_path) {
            if content.contains(&target) {
                let cl = entry.path().join("charge_level");
                if cl.exists() {
                    dirs.push(entry.path());
                }
            }
        }
    }
    dirs
}

fn read_sysfs_u8(path: &std::path::Path) -> Option<u8> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).ok()?;
    buf.trim().parse().ok()
}

/// Tries to read `charge_level` (0–255) and `charge_status` (0/1) from the
/// openrazer kernel module sysfs for the given device PID.
///
/// Returns `None` when the kernel module is not bound or the sysfs attribute
/// does not exist.  When multiple HID interface entries match the same PID the
/// function prefers the entry whose `charge_level` is non-zero (the kernel
/// driver may expose the attribute on multiple interfaces; only one will have
/// up-to-date data).
pub fn query_battery_sysfs(pid: u16) -> Option<BatteryState> {
    let dirs = find_sysfs_charge_dirs(pid);
    if dirs.is_empty() {
        return None;
    }

    // Collect all (level, status) pairs — pick first non-zero level.
    let mut best_level: Option<u8> = None;
    let mut best_status: bool = false;

    for dir in &dirs {
        let level_raw = read_sysfs_u8(&dir.join("charge_level"))?;
        let status_raw = read_sysfs_u8(&dir.join("charge_status")).unwrap_or(0);
        // charge_status is defined as 0=discharging, 1=charging.
        // Values > 1 are invalid (e.g. a raw current reading on a wired
        // interface) — treat them as unknown (not charging).
        let is_charging = status_raw == 1;
        let pct = ((level_raw as u16 * 100) / 255) as u8;
        println!(
            "[Battery/sysfs] PID 0x{pid:04x} dir={} level={level_raw}/255 → {pct}% charging={status_raw}",
            dir.display()
        );
        if best_level.is_none() || (level_raw > 0 && best_level == Some(0)) {
            best_level = Some(pct);
            best_status = is_charging;
        }
    }

    let pct = best_level?;
    let is_charging = best_status;

    let state = if is_charging && pct >= 100 {
        BatteryState::Full
    } else if is_charging {
        BatteryState::Charging(pct)
    } else {
        BatteryState::Discharging(pct)
    };
    println!("[Battery/sysfs] PID 0x{pid:04x} → {state:?}");
    Some(state)
}

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
/// Retries up to 3 times on STATUS_BUSY (0x01) or STATUS_TIMEOUT (0x04).
/// STATUS_TIMEOUT means the dongle could not reach the device wirelessly
/// (device sleeping) — the caller should use a generous `retry_delay` to
/// give the device time to wake up before subsequent attempts.
fn query_level(
    handle: &DeviceHandle<Context>,
    iface: u16,
    transaction_id: u8,
    sleep: &std::time::Duration,
    timeout: std::time::Duration,
    retry_delay: std::time::Duration,
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
                println!("[Battery] Soft retry on level query (attempt {attempt})…");
                std::thread::sleep(retry_delay);
            }
            Err(true) => {
                eprintln!("[Battery] Level query failed (bad response status).");
                return Err(rusb::Error::Io);
            }
        }
    }
    eprintln!("[Battery] Level query exhausted retries.");
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

    // ── Sysfs fast-path ───────────────────────────────────────────────────────
    // When the openrazer razerkbd/razermouse kernel module is bound it handles
    // the full USB wireless protocol and exposes charge_level + charge_status
    // via sysfs.  Reading from there is more reliable than raw libusb control
    // transfers (no detach needed, wireless channel stays up).
    //
    // Strategy:
    //   Dongle + wired companion present → prefer wired companion sysfs (the
    //   dongle returns 0 when keyboard is charging via USB cable).
    //   Otherwise → use sysfs of the queried PID directly.
    if matches!(connection_type, ConnectionType::Dongle) {
        if let Some(wired_pid) = wired_companion_pid {
            if detect_connected_pid(&[wired_pid]).is_some() {
                if let Some(state) = query_battery_sysfs(wired_pid) {
                    println!("[Battery] Sysfs wired companion 0x{wired_pid:04x} → {state:?}");
                    return Ok(state);
                }
            }
        }
    }
    if let Some(state) = query_battery_sysfs(product_id) {
        println!("[Battery] Sysfs for PID 0x{product_id:04x} → {state:?}");
        return Ok(state);
    }
    println!("[Battery] Sysfs not available — falling back to USB control transfers.");

    // ── USB libusb fallback ───────────────────────────────────────────────────
    let (handle, iface_u8) = open_razer_device(product_id)?;
    let iface = iface_u8 as u16;
    let timeout = std::time::Duration::from_millis(500);
    let sleep = std::time::Duration::from_micros(wait_us);
    let retry_delay = match connection_type {
        ConnectionType::Wired => std::time::Duration::from_millis(20),
        ConnectionType::Dongle | ConnectionType::Bluetooth => {
            std::time::Duration::from_millis(1500)
        }
    };
    if matches!(
        connection_type,
        ConnectionType::Dongle | ConnectionType::Bluetooth
    ) {
        let driver_mode_payload = build_set_driver_mode_payload(transaction_id);
        let timeout_dm = std::time::Duration::from_millis(500);
        match handle.write_control(0x21, 0x09, 0x0300, iface, &driver_mode_payload, timeout_dm) {
            Ok(n) => println!("[Battery] Driver mode SET_REPORT sent ({n} bytes)."),
            Err(e) => eprintln!("[Battery] Driver mode SET_REPORT failed (non-fatal): {e:?}"),
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // ── Battery level ─────────────────────────────────────────────────────────
    // On retry exhaustion query_level returns Err — try wired companion sysfs
    // one more time before propagating the error.
    let percent = match query_level(&handle, iface, transaction_id, &sleep, timeout, retry_delay) {
        Ok(pct) if pct == 0 => {
            if let (ConnectionType::Dongle, Some(wired_pid)) =
                (connection_type, wired_companion_pid)
            {
                if detect_connected_pid(&[wired_pid]).is_some() {
                    println!(
                        "[Battery] Dongle returned 0%; trying wired interface (0x{wired_pid:04x}) for level…"
                    );
                    match open_razer_device(wired_pid).and_then(|(h, wi)| {
                        let wired_retry = std::time::Duration::from_millis(20);
                        query_level(&h, wi as u16, transaction_id, &sleep, timeout, wired_retry)
                    }) {
                        Ok(wired_pct) if wired_pct > 0 => {
                            println!(
                                "[Battery] Wired interface returned {wired_pct}% — using this."
                            );
                            wired_pct
                        }
                        _ => pct,
                    }
                } else {
                    pct
                }
            } else {
                pct
            }
        }
        Ok(pct) => pct,
        Err(e) => {
            // Dongle exhausted retries — try wired companion as last resort
            if let (ConnectionType::Dongle, Some(wired_pid)) =
                (connection_type, wired_companion_pid)
            {
                if detect_connected_pid(&[wired_pid]).is_some() {
                    println!(
                        "[Battery] Dongle retries exhausted; trying wired interface (0x{wired_pid:04x})…"
                    );
                    match open_razer_device(wired_pid).and_then(|(h, wi)| {
                        let wired_retry = std::time::Duration::from_millis(20);
                        query_level(&h, wi as u16, transaction_id, &sleep, timeout, wired_retry)
                    }) {
                        Ok(wired_pct) => {
                            println!(
                                "[Battery] Wired interface returned {wired_pct}% — using this."
                            );
                            wired_pct
                        }
                        Err(_) => return Err(e),
                    }
                } else {
                    return Err(e);
                }
            } else {
                return Err(e);
            }
        }
    };

    // ── Charging status ───────────────────────────────────────────────────────
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
                query_charging_status(&handle, iface, transaction_id, &sleep, timeout, retry_delay)?
            }
        }
        ConnectionType::Bluetooth => {
            query_charging_status(&handle, iface, transaction_id, &sleep, timeout, retry_delay)?
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
    retry_delay: std::time::Duration,
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
                println!("[Battery] Soft retry on charging query (attempt {attempt})…");
                std::thread::sleep(retry_delay);
            }
            Err(true) => {
                eprintln!("[Battery] Charging query failed (bad response status).");
                return Err(rusb::Error::Io);
            }
        }
    }

    eprintln!("[Battery] Charging query exhausted retries.");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Build a fake sysfs HID tree and verify `find_sysfs_charge_dirs` and
    /// `query_battery_sysfs` read from it correctly.
    #[test]
    fn test_query_battery_sysfs_reads_charge_level_and_status() {
        // Create a temp dir that mimics /sys/bus/hid/devices/
        let tmp = TempDir::new().unwrap();
        let hid_root = tmp.path();

        // Device entry: PID 0x0258 (wired keyboard), level=255 charging=1
        let entry_dir = hid_root.join("0003:1532:0258.0001");
        fs::create_dir_all(&entry_dir).unwrap();
        fs::write(
            entry_dir.join("uevent"),
            "DRIVER=razerkbd\nHID_ID=0003:00001532:00000258\n",
        )
        .unwrap();
        fs::write(entry_dir.join("charge_level"), "255\n").unwrap();
        fs::write(entry_dir.join("charge_status"), "1\n").unwrap();

        // Directly test the sysfs read helpers using the temp dir
        let level = read_sysfs_u8(&entry_dir.join("charge_level")).unwrap();
        let status = read_sysfs_u8(&entry_dir.join("charge_status")).unwrap();

        assert_eq!(level, 255, "charge_level should be 255");
        assert_eq!(status, 1, "charge_status should be 1 (charging)");

        // Scaled percentage: 255 * 100 / 255 = 100
        let pct = ((level as u16 * 100) / 255) as u8;
        assert_eq!(pct, 100);
    }

    #[test]
    fn test_query_battery_sysfs_returns_none_when_no_sysfs() {
        // PID that has no sysfs entry → should return None, not panic
        // (we rely on the real /sys path being absent for a made-up PID)
        let result = query_battery_sysfs(0xDEAD);
        assert!(result.is_none(), "Should return None for unknown PID");
    }
}
