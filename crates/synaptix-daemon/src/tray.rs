use image::{ImageBuffer, Rgba};
use std::sync::mpsc::Receiver;
use tray_icon::TrayIconBuilder;

/// Battery state update sent from the USB polling loop to the tray.
pub struct TrayUpdate {
    pub device_name: String,
    pub percentage: u8,
    pub is_charging: bool,
}

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

/// Initialises the AppIndicator tray icon and registers a GLib idle/timeout
/// callback to drain battery updates arriving from the tokio polling loop.
///
/// **Must be called from the GTK main thread** before `gtk::main()`.
/// The tray icon will remain visible until the process exits.
pub fn start_tray(rx: Receiver<TrayUpdate>) {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    // libayatana-appindicator3 REQUIRES a menu to show the indicator.
    // Without one the icon is silently suppressed by the AppIndicator protocol.
    let quit_item = MenuItem::new("Quit Synaptix", true, None);
    let menu = Menu::new();
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

    // Poll the channel every second on the GTK main thread.
    // The closure captures both `tray` and `rx`; they live for the process lifetime.
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        // Handle menu events (e.g. "Quit").
        if let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == quit_id {
                gtk::main_quit();
            }
        }

        while let Ok(update) = rx.try_recv() {
            let icon = generate_battery_icon(update.percentage);
            let suffix = if update.is_charging { " ⚡" } else { "" };
            let tooltip = format!("{}: {}%{}", update.device_name, update.percentage, suffix);
            tray.set_icon(Some(icon)).ok();
            tray.set_tooltip(Some(&tooltip)).ok();
        }
        glib::ControlFlow::Continue
    });
}
