use crate::razer_protocol::{RAZER_VID, REPORT_LEN};
use rusb::{Context, UsbContext};

/// Sends a 90-byte HID SET_REPORT control transfer to a Razer device.
///
/// Scans all USB devices for one matching `(VID=0x1532, PID=product_id)`,
/// then issues:
/// ```text
/// bmRequestType = 0x21  (HOST→DEVICE | CLASS | INTERFACE)
/// bRequest      = 0x09  (HID SET_REPORT)
/// wValue        = 0x0300
/// wIndex        = 0x00  (HID interface 0)
/// data          = 90-byte report
/// ```
///
/// Returns `Err(rusb::Error::NoDevice)` when the target device is not present.
pub fn send_control_transfer(product_id: u16, payload: &[u8; REPORT_LEN]) -> rusb::Result<()> {
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

        let handle = match device.open() {
            Ok(h) => { println!("[USB] Device opened successfully."); h }
            Err(e) => {
                eprintln!("[USB] Failed to open device: {e:?}");
                return Err(e);
            }
        };

        // Automatically detach the kernel usbhid driver so we can claim the
        // interface; it is reattached when `handle` is dropped.
        if let Err(e) = handle.set_auto_detach_kernel_driver(true) {
            eprintln!("[USB] set_auto_detach_kernel_driver failed (non-fatal on some kernels): {e:?}");
            // Non-fatal: some kernels/platforms don't support this; continue anyway.
        }

        if let Err(e) = handle.claim_interface(0) {
            eprintln!("[USB] claim_interface(0) failed: {e:?}");
            return Err(e);
        }
        println!("[USB] Interface 0 claimed.");

        let timeout = std::time::Duration::from_millis(500);
        let written = handle.write_control(
            0x21,   // bmRequestType: HOST→DEVICE | CLASS | INTERFACE
            0x09,   // bRequest: HID SET_REPORT
            0x0300, // wValue
            0x00,   // wIndex: interface 0
            payload,
            timeout,
        );

        match written {
            Ok(n) => {
                println!("[USB] write_control returned {n} bytes (expected {REPORT_LEN}).");
                if n != REPORT_LEN {
                    eprintln!("[USB] Short write — expected {REPORT_LEN}, got {n}.");
                    return Err(rusb::Error::Io);
                }
            }
            Err(e) => {
                eprintln!("[USB] write_control failed: {e:?}");
                return Err(e);
            }
        }

        println!("[USB] Control transfer complete.");
        return Ok(());
    }

    eprintln!("[USB] No Razer device with PID 0x{product_id:04x} found.");
    Err(rusb::Error::NoDevice)
}
