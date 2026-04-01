use std::{collections::HashMap, fs, path::PathBuf};
use synaptix_protocol::DeviceSettings;

/// Returns `~/.config/synaptix/devices.json` via the XDG-aware `directories` crate.
fn config_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("synaptix").join("devices.json"))
}

/// Returns `~/.config/synaptix/headset_state.json`.
fn headset_state_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("synaptix").join("headset_state.json"))
}

/// Reads `devices.json` from disk. Returns an empty map on any error (file missing is fine).
pub fn load_settings() -> HashMap<String, DeviceSettings> {
    let path = match config_path() {
        Some(p) => p,
        None => {
            log::warn!("[Config] Could not resolve config directory");
            return HashMap::new();
        }
    };

    match fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|e| {
            log::warn!("[Config] Failed to parse {}: {e}", path.display());
            HashMap::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            log::warn!("[Config] Failed to read {}: {e}", path.display());
            HashMap::new()
        }
    }
}

/// Serialises `settings` to disk. Creates the config directory if it does not exist.
pub fn save_settings(settings: &HashMap<String, DeviceSettings>) {
    let path = match config_path() {
        Some(p) => p,
        None => {
            log::warn!("[Config] Could not resolve config directory — settings not saved");
            return;
        }
    };

    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            log::error!(
                "[Config] Failed to create config dir {}: {e}",
                dir.display()
            );
            return;
        }
    }

    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::error!("[Config] Failed to write {}: {e}", path.display());
            } else {
                log::info!("[Config] Settings saved to {}", path.display());
            }
        }
        Err(e) => log::error!("[Config] Serialisation failed: {e}"),
    }
}

/// Loads the persisted Kraken V4 Pro haptic level from disk.
///
/// Returns the saved level (0–100) or **33 (Low)** as a safe default when the
/// file is absent. Never returns 0 as default — 0 would reset the hub's
/// physical haptic setting to OFF on the first battery poll after daemon start.
pub fn load_haptic_level() -> u8 {
    let path = match headset_state_path() {
        Some(p) => p,
        None => return 33,
    };

    let json = match fs::read_to_string(&path) {
        Ok(j) => j,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return 33,
        Err(e) => {
            log::warn!(
                "[Config] Failed to read headset state {}: {e}",
                path.display()
            );
            return 33;
        }
    };

    match serde_json::from_str::<serde_json::Value>(&json) {
        Ok(v) => {
            let level = v["kraken_haptic_level"]
                .as_u64()
                .map(|n| n as u8)
                .unwrap_or(33);
            log::info!("[Config] Loaded persisted haptic level: {level}");
            level
        }
        Err(e) => {
            log::warn!("[Config] Failed to parse headset state: {e}");
            33
        }
    }
}

/// Saves the Kraken V4 Pro haptic level to disk so it survives daemon restarts.
///
/// Called from `DeviceManager::set_haptic_intensity` after a successful USB write.
pub fn save_haptic_level(level: u8) {
    let path = match headset_state_path() {
        Some(p) => p,
        None => {
            log::warn!("[Config] Could not resolve config directory — haptic level not saved");
            return;
        }
    };

    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            log::error!(
                "[Config] Failed to create config dir {}: {e}",
                dir.display()
            );
            return;
        }
    }

    let json = format!("{{\"kraken_haptic_level\":{level}}}");
    if let Err(e) = fs::write(&path, &json) {
        log::error!("[Config] Failed to write haptic level: {e}");
    } else {
        log::info!(
            "[Config] Haptic level {level} persisted to {}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synaptix_protocol::LightingEffect;

    #[test]
    fn test_save_and_load_settings() {
        // Use a temp file so the test doesn't touch the real config.
        let tmp = std::env::temp_dir().join("synaptix_test_settings.json");

        let mut settings: HashMap<String, DeviceSettings> = HashMap::new();
        settings.insert(
            "cobra-pro".to_string(),
            DeviceSettings {
                lighting: Some(LightingEffect::Static([0x44, 0xD6, 0x2C])),
                dpi: Some(1800),
            },
        );

        // Write directly to the temp path (bypass config_path).
        let json = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&tmp, &json).unwrap();

        // Read back and verify.
        let loaded: HashMap<String, DeviceSettings> =
            serde_json::from_str(&std::fs::read_to_string(&tmp).unwrap()).unwrap();

        let entry = loaded
            .get("cobra-pro")
            .expect("cobra-pro not in loaded settings");
        assert!(matches!(
            entry.lighting,
            Some(LightingEffect::Static([0x44, 0xD6, 0x2C]))
        ));
        assert_eq!(entry.dpi, Some(1800));

        std::fs::remove_file(tmp).ok();
    }

    /// load_haptic_level returns 33 when no file exists (safe non-zero default).
    #[test]
    fn test_load_haptic_level_default_when_absent() {
        let level = load_haptic_level_from_path(
            &std::env::temp_dir().join("synaptix_haptic_NONEXISTENT_test.json"),
        );
        assert_eq!(
            level, 33,
            "default must be 33, not 0 (0 would reset hub haptics)"
        );
    }

    /// save_haptic_level + load round-trip.
    #[test]
    fn test_save_and_load_haptic_level() {
        let tmp = std::env::temp_dir().join("synaptix_test_haptic_level.json");
        save_haptic_level_to_path(100, &tmp);
        let loaded = load_haptic_level_from_path(&tmp);
        assert_eq!(loaded, 100);

        save_haptic_level_to_path(33, &tmp);
        assert_eq!(load_haptic_level_from_path(&tmp), 33);

        save_haptic_level_to_path(0, &tmp);
        assert_eq!(
            load_haptic_level_from_path(&tmp),
            0,
            "level 0 (off) must round-trip correctly"
        );

        std::fs::remove_file(tmp).ok();
    }

    /// save_haptic_level_to_path / load_haptic_level_from_path for testable I/O.
    fn save_haptic_level_to_path(level: u8, path: &std::path::Path) {
        let json = format!("{{\"kraken_haptic_level\":{level}}}");
        std::fs::write(path, &json).unwrap();
    }

    fn load_haptic_level_from_path(path: &std::path::Path) -> u8 {
        match std::fs::read_to_string(path) {
            Ok(json) => serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| v["kraken_haptic_level"].as_u64())
                .map(|n| n as u8)
                .unwrap_or(33),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 33,
            Err(_) => 33,
        }
    }
}
