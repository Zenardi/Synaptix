//! synaptix-probe — USB protocol discovery tool for Razer Kraken V4 Pro
//!
//! Sends 64-byte HID reports to Razer headset PIDs and sweeps command IDs
//! so you can listen for audio/haptic changes and identify the correct
//! protocol bytes.
//!
//! Usage:
//!   # Single shot — send one specific report
//!   sudo synaptix-probe --pid 0x0568 --cmd-id 0x01 --value 100
//!
//!   # Sweep all cmd IDs on both known V4 Pro PIDs
//!   sudo synaptix-probe --sweep
//!
//!   # Slower sweep with 3s between attempts (easier to hear changes)
//!   sudo synaptix-probe --sweep --interval 3000

use clap::Parser;
use rusb::{Context, UsbContext};
use std::time::Duration;

const RAZER_VID: u16 = 0x1532;

/// Known Kraken V4 Pro PIDs to probe
const V4PRO_PIDS: &[u16] = &[0x0568, 0x056c];

/// USB SET_REPORT control transfer parameters for the 64-byte V4 Pro protocol
const REQ_TYPE_OUT: u8 = 0x21; // Host→Device | Class | Interface
const REQ_SET_REPORT: u8 = 0x09;
const W_VALUE: u16 = 0x0202; // Output report, report ID 2
const W_INDEX: u16 = 0x0004; // Interface 4
const TIMEOUT_MS: u64 = 1000;

/// 64-byte report header fields (confirmed from haptics reverse-engineering)
const REPORT_ID: u8 = 0x21;
const CMD_CLASS: u8 = 0x0F; // DSP / audio subsystem
const ROUTING: u8 = 0x80; // Route to headset DSP

#[derive(Parser)]
#[command(
    name = "synaptix-probe",
    about = "USB protocol probe for Razer Kraken V4 Pro volume/audio discovery"
)]
struct Cli {
    /// Target PID in hex (e.g. 0x0568). Omit to try all known V4 Pro PIDs.
    #[arg(long, value_parser = parse_hex_u16)]
    pid: Option<u16>,

    /// Command ID to send (buf[2]) in hex, e.g. 0x01
    #[arg(long, value_parser = parse_hex_u8)]
    cmd_id: Option<u8>,

    /// Payload value at buf[29] (0-100)
    #[arg(long, default_value = "100")]
    value: u8,

    /// Sweep mode: iterate cmd_id 0x00-0x0F with values 0/50/100
    #[arg(long)]
    sweep: bool,

    /// Milliseconds to wait between sweep attempts
    #[arg(long, default_value = "2000")]
    interval: u64,

    /// Interface number to claim (default 4 for V4 Pro)
    #[arg(long, default_value = "4")]
    interface: u8,
}

fn parse_hex_u16(s: &str) -> Result<u16, String> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u16::from_str_radix(s, 16).map_err(|e| e.to_string())
}

fn parse_hex_u8(s: &str) -> Result<u8, String> {
    let s = s.trim_start_matches("0x").trim_start_matches("0X");
    u8::from_str_radix(s, 16).map_err(|e| e.to_string())
}

/// Build a 64-byte report with the V4 Pro DSP header.
///
/// buf[0]  = 0x21 Report ID
/// buf[1]  = 0x0F Command Class (DSP)
/// buf[2]  = cmd_id
/// buf[3]  = 0x80 Routing (headset DSP)
/// buf[29] = payload value
/// buf[62] = XOR checksum of bytes[1..62]
fn build_report(cmd_id: u8, value: u8, data_offset: usize) -> [u8; 64] {
    let mut buf = [0u8; 64];
    buf[0] = REPORT_ID;
    buf[1] = CMD_CLASS;
    buf[2] = cmd_id;
    buf[3] = ROUTING;
    buf[data_offset] = value;

    // XOR checksum over bytes 1..62
    let crc: u8 = buf[1..62].iter().fold(0u8, |acc, &b| acc ^ b);
    buf[62] = crc;

    buf
}

/// Open the Razer device with the given PID, detach kernel driver, claim interface.
fn open_device(pid: u16, interface: u8) -> Option<rusb::DeviceHandle<Context>> {
    let ctx = Context::new().ok()?;
    let devices = ctx.devices().ok()?;

    for device in devices.iter() {
        let desc = device.device_descriptor().ok()?;
        if desc.vendor_id() == RAZER_VID && desc.product_id() == pid {
            let handle = device.open().ok()?;
            let _ = handle.set_auto_detach_kernel_driver(true);
            handle.claim_interface(interface).ok()?;
            return Some(handle);
        }
    }
    None
}

fn send_report(
    handle: &rusb::DeviceHandle<Context>,
    report: &[u8; 64],
) -> Result<usize, rusb::Error> {
    handle.write_control(
        REQ_TYPE_OUT,
        REQ_SET_REPORT,
        W_VALUE,
        W_INDEX,
        report,
        Duration::from_millis(TIMEOUT_MS),
    )
}

fn probe_once(pid: u16, cmd_id: u8, value: u8, data_offset: usize, interface: u8) {
    let report = build_report(cmd_id, value, data_offset);
    println!(
        "  [probe] pid=0x{pid:04x} cmd_id=0x{cmd_id:02x} value={value:3} offset={data_offset} — bytes: {:02x?}",
        &report[0..10]
    );

    match open_device(pid, interface) {
        None => println!("  [probe] ⚠ Device 0x{pid:04x} not found or could not be opened"),
        Some(handle) => match send_report(&handle, &report) {
            Ok(n) => println!("  [probe] ✓ Sent {n} bytes"),
            Err(rusb::Error::Timeout) => println!("  [probe] ✗ TIMEOUT"),
            Err(rusb::Error::Pipe) => println!("  [probe] ✗ PIPE (command rejected by device)"),
            Err(e) => println!("  [probe] ✗ Error: {e}"),
        },
    }
}

fn run_sweep(pids: &[u16], interval_ms: u64, interface: u8) {
    // Skip 0x03 (haptics — already known)
    let cmd_ids: Vec<u8> = (0x00u8..=0x0Fu8).filter(|&id| id != 0x03).collect();
    let values = [0u8, 50u8, 100u8];
    let data_offsets = [29usize]; // Start with buf[29] (same as haptics)

    let total = pids.len() * cmd_ids.len() * values.len() * data_offsets.len();
    println!("=== synaptix-probe sweep ===");
    println!(
        "PIDs: {:?} | cmd_ids: {} | values: {:?} | offset(s): {:?}",
        pids,
        cmd_ids.len(),
        values,
        data_offsets
    );
    println!("Total attempts: {total} | Interval: {interval_ms}ms");
    println!("Listen for audio level changes. Note the cmd_id when you hear a change.\n");

    let mut attempt = 0usize;
    for &pid in pids {
        for &cmd_id in &cmd_ids {
            for &offset in &data_offsets {
                for &value in &values {
                    attempt += 1;
                    println!(
                        "─── Attempt {attempt}/{total} | pid=0x{pid:04x} cmd_id=0x{cmd_id:02x} \
                         offset={offset} value={value} ───"
                    );
                    probe_once(pid, cmd_id, value, offset, interface);
                    if attempt < total {
                        std::thread::sleep(Duration::from_millis(interval_ms));
                    }
                }
            }
        }
    }

    println!("\n=== Sweep complete ===");
    println!("If you heard a volume change, note the cmd_id and re-run with --cmd-id to confirm.");
    println!("Example: sudo synaptix-probe --pid 0x0568 --cmd-id 0x05 --value 0  (to set to 0)");
    println!("         sudo synaptix-probe --pid 0x0568 --cmd-id 0x05 --value 100 (to set to max)");
}

fn main() {
    let cli = Cli::parse();

    if cli.sweep {
        let pids: Vec<u16> = match cli.pid {
            Some(p) => vec![p],
            None => V4PRO_PIDS.to_vec(),
        };
        run_sweep(&pids, cli.interval, cli.interface);
    } else {
        let cmd_id = cli.cmd_id.unwrap_or_else(|| {
            eprintln!("Error: --cmd-id is required in single-shot mode (or use --sweep)");
            std::process::exit(1);
        });
        let pids: Vec<u16> = match cli.pid {
            Some(p) => vec![p],
            None => V4PRO_PIDS.to_vec(),
        };
        for &pid in &pids {
            probe_once(pid, cmd_id, cli.value, 29, cli.interface);
        }
    }
}
