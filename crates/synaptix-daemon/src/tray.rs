use image::{ImageBuffer, Rgba};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;
use tray_icon::TrayIconBuilder;

/// Battery state update sent from the USB polling loop to the tray.
pub struct TrayUpdate {
    /// Stable identifier, e.g. `"cobra-pro"` or `"kraken-v4-pro-0568"`.
    pub device_id: String,
    pub device_name: String,
    pub percentage: u8,
    pub is_charging: bool,
}

// ── Pure helper functions (testable without GTK) ─────────────────────────────

/// Returns the lowest battery percentage across all tracked devices.
/// Returns 0 when the map is empty.
pub fn lowest_battery_pct(devices: &HashMap<String, TrayUpdate>) -> u8 {
    devices.values().map(|u| u.percentage).min().unwrap_or(0)
}

/// Formats a single device label for the tray menu, e.g. `"Razer Cobra Pro: 75% ⚡"`.
pub fn build_device_label(update: &TrayUpdate) -> String {
    let suffix = if update.is_charging { " ⚡" } else { "" };
    format!("{}: {}%{}", update.device_name, update.percentage, suffix)
}

/// Formats a combined tooltip when multiple devices are present,
/// joining their individual labels with `" | "`.
pub fn build_combined_tooltip(devices: &HashMap<String, TrayUpdate>) -> String {
    if devices.is_empty() {
        return "Synaptix – Scanning…".to_string();
    }
    // Stable sort by device_id so the order doesn't jump around.
    let mut entries: Vec<&TrayUpdate> = devices.values().collect();
    entries.sort_by_key(|u| u.device_id.as_str());
    entries
        .iter()
        .map(|u| build_device_label(u))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Percentage thresholds that trigger a low-battery desktop notification.
const LOW_BATTERY_THRESHOLDS: &[u8] = &[20, 15, 10, 5, 1];

/// Generates a 32×32 RGBA battery icon filled proportionally.
///
/// The battery body is a horizontal capsule (left = empty, right = positive
/// terminal cap). The fill goes left-to-right from 0 % to 100 %.
///
/// * > 40 % → Razer green  (`#44D62C`)
/// * 20–40 % → amber       (`#FFC800`)
/// * ≤ 20 %  → red         (`#FF3C3C`)
pub fn generate_battery_icon(percentage: u8) -> tray_icon::Icon {
    const W: u32 = 32;
    const H: u32 = 32;

    // Colour palette
    let green = Rgba([0x44u8, 0xD6, 0x2C, 255]);
    let amber = Rgba([0xFF, 0xC8, 0x00, 255]);
    let red = Rgba([0xFF, 0x3C, 0x3C, 255]);
    let white = Rgba([0xFF, 0xFF, 0xFF, 255]);
    let transparent = Rgba([0u8, 0, 0, 0]);

    let fill_color = if percentage <= 20 {
        red
    } else if percentage <= 40 {
        amber
    } else {
        green
    };

    // Battery body region (exclusive right/bottom walls):
    //   body  : x ∈ [2, 27], y ∈ [10, 21]  (26 px wide, 12 px tall)
    //   cap   : x ∈ [28, 30], y ∈ [13, 18]  (positive terminal, 3 px wide)
    //   inner : x ∈ [3, 26], y ∈ [11, 20]   (fill area, 24 × 10 px)
    let body_x1: u32 = 2;
    let body_x2: u32 = 27;
    let body_y1: u32 = 10;
    let body_y2: u32 = 21;

    let inner_x1: u32 = body_x1 + 1;
    let inner_x2: u32 = body_x2; // right wall is exclusive
    let inner_y1: u32 = body_y1 + 1;
    let inner_y2: u32 = body_y2; // bottom wall is exclusive
    let inner_width: u32 = inner_x2 - inner_x1;

    let fill_px = (inner_width * percentage as u32 / 100).max(if percentage > 0 { 1 } else { 0 });

    let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::new(W, H);

    for y in 0..H {
        for x in 0..W {
            let in_body = x >= body_x1 && x <= body_x2 && y >= body_y1 && y <= body_y2;
            let on_border =
                in_body && (x == body_x1 || x == body_x2 || y == body_y1 || y == body_y2);
            let in_fill = x >= inner_x1 && x < inner_x1 + fill_px && y >= inner_y1 && y < inner_y2;
            // Positive terminal cap: 3 px wide, vertically centred
            let cap_x = body_x2 + 1..=body_x2 + 3;
            let in_cap = cap_x.contains(&x) && (13..=18).contains(&y);

            let pixel = if on_border || in_cap {
                white
            } else if in_fill {
                fill_color
            } else {
                transparent
            };

            img.put_pixel(x, y, pixel);
        }
    }

    tray_icon::Icon::from_rgba(img.into_raw(), W, H).expect("icon buffer is valid")
}

/// Sends a desktop notification for a low-battery threshold crossing.
fn notify_low_battery(device_name: &str, percentage: u8) {
    use notify_rust::{Notification, Urgency};

    let urgency = if percentage <= 10 {
        Urgency::Critical
    } else {
        Urgency::Normal
    };
    let icon = if percentage <= 10 {
        "battery-empty"
    } else {
        "battery-caution"
    };

    if let Err(e) = Notification::new()
        .summary(&format!("Low Battery — {device_name}"))
        .body(&format!("Battery level is at {percentage}%."))
        .icon(icon)
        .urgency(urgency)
        .timeout(notify_rust::Timeout::Milliseconds(8_000))
        .show()
    {
        eprintln!("[Tray] Failed to send low-battery notification: {e:?}");
    }
}

/// Initialises the AppIndicator tray icon and registers a GLib idle/timeout
/// callback to drain battery updates arriving from the tokio polling loop.
///
/// Handles **multiple devices** — one menu item per device, tray icon reflects
/// the device with the lowest battery percentage.
///
/// **Must be called from the GTK main thread** before `gtk::main()`.
/// The tray icon will remain visible until the process exits.
pub fn start_tray(rx: Receiver<TrayUpdate>) {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let quit_item = MenuItem::new("Quit Synaptix", true, None);

    let menu = Menu::new();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&quit_item).ok();

    // Clone the menu handle before passing ownership to TrayIconBuilder so we
    // can still call menu.insert() later when new devices appear.
    let tray = TrayIconBuilder::new()
        .with_icon(generate_battery_icon(0))
        .with_tooltip("Synaptix – Scanning…")
        .with_menu(Box::new(menu.clone()))
        .build()
        .expect("failed to build AppIndicator tray icon");

    let quit_id = quit_item.id().clone();

    // Per-device state: battery level and notification tracking.
    let mut devices: HashMap<String, TrayUpdate> = HashMap::new();
    // device_id → set of thresholds already notified this discharge cycle.
    let mut notified: HashMap<String, HashSet<u8>> = HashMap::new();
    // device_id → last known percentage (to detect downward crossings).
    let mut last_pct: HashMap<String, u8> = HashMap::new();
    // Menu items we've inserted, keyed by device_id.
    let mut device_items: HashMap<String, MenuItem> = HashMap::new();

    // Poll the channel every second on the GTK main thread.
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                gtk::main_quit();
            }
        }

        let mut any_update = false;

        while let Ok(update) = rx.try_recv() {
            any_update = true;
            let id = update.device_id.clone();

            // Insert a new menu item the first time we see this device.
            if !device_items.contains_key(&id) {
                let label = build_device_label(&update);
                let item = MenuItem::new(&label, false, None);
                // Insert before the separator (index 0).
                menu.insert(&item, 0).ok();
                device_items.insert(id.clone(), item);
            } else if let Some(item) = device_items.get(&id) {
                item.set_text(build_device_label(&update));
            }

            // Low-battery notifications (discharging only).
            if !update.is_charging {
                let pct = update.percentage;
                let fired = notified.entry(id.clone()).or_default();
                let prev = last_pct.get(&id).copied();

                for &threshold in LOW_BATTERY_THRESHOLDS {
                    if pct <= threshold
                        && !fired.contains(&threshold)
                        && prev.is_none_or(|p| p > threshold)
                    {
                        notify_low_battery(&update.device_name, pct);
                        fired.insert(threshold);
                    }
                }
                fired.retain(|&t| pct <= t);
            } else {
                notified.entry(id.clone()).or_default().clear();
            }

            last_pct.insert(id.clone(), update.percentage);
            devices.insert(id, update);
        }

        if any_update {
            let lowest = lowest_battery_pct(&devices);
            tray.set_icon(Some(generate_battery_icon(lowest))).ok();
            tray.set_tooltip(Some(&build_combined_tooltip(&devices)))
                .ok();
        }

        glib::ControlFlow::Continue
    });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_update(device_id: &str, device_name: &str, pct: u8, charging: bool) -> TrayUpdate {
        TrayUpdate {
            device_id: device_id.to_string(),
            device_name: device_name.to_string(),
            percentage: pct,
            is_charging: charging,
        }
    }

    #[test]
    fn test_build_device_label_discharging() {
        let u = make_update("cobra-pro", "Razer Cobra Pro", 75, false);
        assert_eq!(build_device_label(&u), "Razer Cobra Pro: 75%");
    }

    #[test]
    fn test_build_device_label_charging() {
        let u = make_update("cobra-pro", "Razer Cobra Pro", 80, true);
        assert_eq!(build_device_label(&u), "Razer Cobra Pro: 80% ⚡");
    }

    #[test]
    fn test_lowest_battery_pct_single() {
        let mut map = HashMap::new();
        map.insert(
            "cobra-pro".to_string(),
            make_update("cobra-pro", "Cobra Pro", 42, false),
        );
        assert_eq!(lowest_battery_pct(&map), 42);
    }

    #[test]
    fn test_lowest_battery_pct_multi() {
        let mut map = HashMap::new();
        map.insert(
            "cobra-pro".to_string(),
            make_update("cobra-pro", "Cobra Pro", 60, false),
        );
        map.insert(
            "kraken-v4-pro".to_string(),
            make_update("kraken-v4-pro", "Kraken V4 Pro", 15, false),
        );
        assert_eq!(lowest_battery_pct(&map), 15);
    }

    #[test]
    fn test_lowest_battery_pct_empty() {
        let map: HashMap<String, TrayUpdate> = HashMap::new();
        assert_eq!(lowest_battery_pct(&map), 0);
    }

    #[test]
    fn test_build_combined_tooltip_single() {
        let mut map = HashMap::new();
        map.insert(
            "cobra-pro".to_string(),
            make_update("cobra-pro", "Cobra Pro", 75, false),
        );
        assert_eq!(build_combined_tooltip(&map), "Cobra Pro: 75%");
    }

    #[test]
    fn test_build_combined_tooltip_multi() {
        let mut map = HashMap::new();
        map.insert(
            "cobra-pro".to_string(),
            make_update("cobra-pro", "Cobra Pro", 60, false),
        );
        map.insert(
            "kraken-v4-pro".to_string(),
            make_update("kraken-v4-pro", "Kraken V4 Pro", 96, false),
        );
        // Sorted by device_id: cobra-pro < kraken-v4-pro
        assert_eq!(
            build_combined_tooltip(&map),
            "Cobra Pro: 60% | Kraken V4 Pro: 96%"
        );
    }

    #[test]
    fn test_build_combined_tooltip_empty() {
        let map: HashMap<String, TrayUpdate> = HashMap::new();
        assert_eq!(build_combined_tooltip(&map), "Synaptix – Scanning…");
    }
}
