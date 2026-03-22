use std::{collections::HashMap, fs, path::PathBuf};
use synaptix_protocol::DeviceSettings;

/// Returns `~/.config/synaptix/devices.json` via the XDG-aware `directories` crate.
fn config_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|b| b.config_dir().join("synaptix").join("devices.json"))
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
}
