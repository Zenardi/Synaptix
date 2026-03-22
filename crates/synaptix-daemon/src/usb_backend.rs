use crate::razer_protocol::{
    build_battery_query_payload, build_charging_query_payload, RAZER_VID, REPORT_LEN,
};
use rusb::{Context, DeviceHandle, UsbContext};
use synaptix_protocol::{BatteryState, ConnectionType};

/// PID of the Cobra Pro wired interface (USB cable). Used to detect whether the
/// cable is plugged in even when the active connection is via the dongle.
const COBRA_PRO_WIRED_PID: u16 = 0x00AF;

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Opens the first Razer device matching `product_id` and returns its handle
/// with kernel driver detached and interface 0 claimed.
fn open_razer_device(product_id: u16) -> rusb::Result<DeviceHandle<Context>> {
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

        handle.claim_interface(0).inspect_err(|e| {
            eprintln!("[USB] claim_interface(0) failed: {e:?}");
        })?;

        println!("[USB] Interface 0 claimed.");
        return Ok(handle);
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
/// wIndex        = 0x00  (HID interface 0)
/// data          = 90-byte report
/// ```
pub fn send_control_transfer(product_id: u16, payload: &[u8; REPORT_LEN]) -> rusb::Result<()> {
    let handle = open_razer_device(product_id)?;
    let timeout = std::time::Duration::from_millis(500);

    let n = handle
        .write_control(0x21, 0x09, 0x0300, 0x00, payload, timeout)
        .inspect_err(|e| eprintln!("[USB] write_control failed: {e:?}"))?;

    println!("[USB] write_control returned {n} bytes (expected {REPORT_LEN}).");
    if n != REPORT_LEN {
        eprintln!("[USB] Short write — expected {REPORT_LEN}, got {n}.");
        return Err(rusb::Error::Io);
    }

    println!("[USB] Control transfer complete.");
    Ok(())
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

    let handle = open_razer_device(product_id)?;
    let timeout = std::time::Duration::from_millis(500);
    let sleep = std::time::Duration::from_micros(wait_us);

    // ── Battery level ─────────────────────────────────────────────────────────
    let level_query = build_battery_query_payload(transaction_id);

    handle
        .write_control(0x21, 0x09, 0x0300, 0x00, &level_query, timeout)
        .inspect_err(|e| eprintln!("[Battery] SET_REPORT (level) failed: {e:?}"))?;

    std::thread::sleep(sleep);

    let mut level_response = [0u8; REPORT_LEN];
    handle
        .read_control(0xA1, 0x01, 0x0300, 0x00, &mut level_response, timeout)
        .inspect_err(|e| eprintln!("[Battery] GET_REPORT (level) failed: {e:?}"))?;

    let raw_level = level_response[9];
    let percent = crate::razer_protocol::parse_battery_response(&level_response);
    println!("[Battery] Raw level: {raw_level}/255 → {percent}%");

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
                query_charging_status(&handle, transaction_id, &sleep, timeout)?
            }
        }
        ConnectionType::Bluetooth => {
            query_charging_status(&handle, transaction_id, &sleep, timeout)?
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
    transaction_id: u8,
    sleep: &std::time::Duration,
    timeout: std::time::Duration,
) -> rusb::Result<bool> {
    let charging_query = build_charging_query_payload(transaction_id);

    handle
        .write_control(0x21, 0x09, 0x0300, 0x00, &charging_query, timeout)
        .inspect_err(|e| eprintln!("[Battery] SET_REPORT (charging) failed: {e:?}"))?;

    std::thread::sleep(*sleep);

    let mut charging_response = [0u8; REPORT_LEN];
    handle
        .read_control(0xA1, 0x01, 0x0300, 0x00, &mut charging_response, timeout)
        .inspect_err(|e| eprintln!("[Battery] GET_REPORT (charging) failed: {e:?}"))?;

    Ok(charging_response[9] != 0)
}
