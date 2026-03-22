use image::{ImageBuffer, Rgba};
use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use tray_icon::TrayIconBuilder;

/// Battery state update sent from the USB polling loop to the tray.
pub struct TrayUpdate {
    pub device_name: String,
    pub percentage: u8,
    pub is_charging: bool,
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
/// **Must be called from the GTK main thread** before `gtk::main()`.
/// The tray icon will remain visible until the process exits.
pub fn start_tray(rx: Receiver<TrayUpdate>) {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    // A non-clickable label at the top of the menu showing current battery state.
    let status_item = MenuItem::new("Synaptix – Scanning…", false, None);
    let quit_item = MenuItem::new("Quit Synaptix", true, None);

    // libayatana-appindicator3 REQUIRES a menu to show the indicator.
    let menu = Menu::new();
    menu.append(&status_item).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&quit_item).ok();

    let icon = generate_battery_icon(0);

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("Synaptix – Scanning…")
        .with_menu(Box::new(menu))
        .build()
        .expect("failed to build AppIndicator tray icon");

    let quit_id = quit_item.id().clone();

    // Track which thresholds have already fired to avoid spamming.
    let mut notified: HashSet<u8> = HashSet::new();
    let mut last_pct: Option<u8> = None;

    // Poll the channel every second on the GTK main thread.
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        // Handle menu events.
        if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                gtk::main_quit();
            }
        }

        while let Ok(update) = rx.try_recv() {
            let suffix = if update.is_charging { " ⚡" } else { "" };
            let label = format!("{}: {}%{}", update.device_name, update.percentage, suffix);

            // Update icon, tooltip, and the status label shown in the menu.
            tray.set_icon(Some(generate_battery_icon(update.percentage)))
                .ok();
            tray.set_tooltip(Some(&label)).ok();
            status_item.set_text(&label);

            // ── Low-battery notifications (discharging only) ──────────────
            if !update.is_charging {
                let pct = update.percentage;
                for &threshold in LOW_BATTERY_THRESHOLDS {
                    // Fire when crossing a threshold downward for the first time.
                    if pct <= threshold
                        && !notified.contains(&threshold)
                        && last_pct.map_or(true, |prev| prev > threshold)
                    {
                        notify_low_battery(&update.device_name, pct);
                        notified.insert(threshold);
                    }
                }
                // If battery has risen above a threshold, re-arm it.
                notified.retain(|&t| pct <= t);
            } else {
                // Charging — reset so notifications re-arm for the next discharge.
                notified.clear();
            }

            last_pct = Some(update.percentage);
        }
        glib::ControlFlow::Continue
    });
}
