//! Frozen v1 config schema, atomic save, defensive load.
//!
//! A missing or corrupt file never fails the app: it falls back to defaults and
//! reports one warning line for the caller to log. Values outside the safe
//! ranges are clamped on load (and on every mutation) rather than rejected.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gesture::{self, Params};

pub const CONFIG_VERSION: u32 = 1;

pub const FINGER_MIN: i64 = 2;
pub const FINGER_MAX: i64 = 5;
pub const DURATION_MIN_MS: i64 = 150;
pub const DURATION_MAX_MS: i64 = 800;
pub const MOVEMENT_MIN: f32 = 0.02;
pub const MOVEMENT_MAX: f32 = 0.20;
pub const COOLDOWN_MIN_MS: i64 = 100;
pub const COOLDOWN_MAX_MS: i64 = 1000;

pub const ACTION_MIDDLE_CLICK: &str = "middle_click";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub enabled: bool,
    pub gesture: GestureConfig,
    pub launch_at_login: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureConfig {
    /// i64 rather than u8/u64 so an out-of-range number clamps instead of
    /// failing the whole document during deserialization.
    pub finger_count: i64,
    pub action: String,
    pub max_duration_ms: i64,
    pub max_movement: f32,
    pub cooldown_ms: i64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            enabled: true,
            gesture: GestureConfig::default(),
            launch_at_login: false,
        }
    }
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            finger_count: 3,
            action: ACTION_MIDDLE_CLICK.to_string(),
            max_duration_ms: 400,
            max_movement: 0.08,
            cooldown_ms: 250,
        }
    }
}

impl Config {
    /// Clamp every value into its documented range. Returns human readable
    /// warnings for each value that had to be adjusted (caller logs them).
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.version != CONFIG_VERSION {
            warnings.push(format!(
                "config version {} is not supported, using version {CONFIG_VERSION}",
                self.version
            ));
            self.version = CONFIG_VERSION;
        }

        let g = &mut self.gesture;

        if !(FINGER_MIN..=FINGER_MAX).contains(&g.finger_count) {
            warnings.push(format!(
                "gesture.finger_count {} clamped to 3",
                g.finger_count
            ));
            g.finger_count = 3;
        }

        if !(DURATION_MIN_MS..=DURATION_MAX_MS).contains(&g.max_duration_ms) {
            warnings.push(format!(
                "gesture.max_duration_ms {} clamped to {}..={}",
                g.max_duration_ms, DURATION_MIN_MS, DURATION_MAX_MS
            ));
            g.max_duration_ms = g.max_duration_ms.clamp(DURATION_MIN_MS, DURATION_MAX_MS);
        }

        if !g.max_movement.is_finite() {
            warnings.push("gesture.max_movement is not a finite number, using 0.08".to_string());
            g.max_movement = 0.08;
        } else if !(MOVEMENT_MIN..=MOVEMENT_MAX).contains(&g.max_movement) {
            warnings.push(format!(
                "gesture.max_movement {} clamped to {}..={}",
                g.max_movement, MOVEMENT_MIN, MOVEMENT_MAX
            ));
            g.max_movement = g.max_movement.clamp(MOVEMENT_MIN, MOVEMENT_MAX);
        }

        if !(COOLDOWN_MIN_MS..=COOLDOWN_MAX_MS).contains(&g.cooldown_ms) {
            warnings.push(format!(
                "gesture.cooldown_ms {} clamped to {}..={}",
                g.cooldown_ms, COOLDOWN_MIN_MS, COOLDOWN_MAX_MS
            ));
            g.cooldown_ms = g.cooldown_ms.clamp(COOLDOWN_MIN_MS, COOLDOWN_MAX_MS);
        }

        if g.action != ACTION_MIDDLE_CLICK {
            warnings.push(format!(
                "gesture.action {:?} is not supported yet, using \"{ACTION_MIDDLE_CLICK}\"",
                g.action
            ));
            g.action = ACTION_MIDDLE_CLICK.to_string();
        }

        warnings
    }

    /// Project the gesture block onto the pure state machine parameters.
    pub fn params(&self) -> Params {
        Params {
            finger_count: self.gesture.finger_count.clamp(FINGER_MIN, FINGER_MAX) as u8,
            max_duration_ms: self
                .gesture
                .max_duration_ms
                .clamp(DURATION_MIN_MS, DURATION_MAX_MS) as u64,
            max_movement: if self.gesture.max_movement.is_finite() {
                self.gesture.max_movement.clamp(MOVEMENT_MIN, MOVEMENT_MAX)
            } else {
                Params::default().max_movement
            },
            cooldown_ms: self
                .gesture
                .cooldown_ms
                .clamp(COOLDOWN_MIN_MS, COOLDOWN_MAX_MS) as u64,
        }
    }
}

impl GestureConfig {
    pub fn from_params(params: gesture::Params) -> Self {
        let p = params.sanitized();
        Self {
            finger_count: i64::from(p.finger_count),
            action: ACTION_MIDDLE_CLICK.to_string(),
            max_duration_ms: p.max_duration_ms as i64,
            max_movement: p.max_movement,
            cooldown_ms: p.cooldown_ms as i64,
        }
    }
}

/// Load a config file. Never fails: on any problem it returns defaults plus a
/// single warning line describing the fallback.
pub fn load(path: &Path) -> (Config, Vec<String>) {
    let mut warnings = Vec::new();

    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (Config::default(), warnings);
        }
        Err(err) => {
            warnings.push(format!(
                "could not read {} ({err}), using defaults",
                path.display()
            ));
            return (Config::default(), warnings);
        }
    };

    let mut config: Config = match serde_json::from_str(&raw) {
        Ok(config) => config,
        Err(err) => {
            warnings.push(format!(
                "could not parse {} ({err}), using defaults",
                path.display()
            ));
            return (Config::default(), warnings);
        }
    };

    warnings.extend(config.sanitize());
    (config, warnings)
}

/// Save atomically: write `config.json.tmp`, then rename over the real file.
pub fn save(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create config dir {}: {err}", parent.display()))?;
        }
    }

    let tmp = tmp_path(path);
    let json =
        serde_json::to_string_pretty(config).map_err(|err| format!("serialize config: {err}"))?;

    std::fs::write(&tmp, json.as_bytes())
        .map_err(|err| format!("write {}: {err}", tmp.display()))?;

    if let Err(err) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "rename {} -> {}: {err}",
            tmp.display(),
            path.display()
        ));
    }

    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_file() -> PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir()
            .join(format!("trio-config-test-{}-{id}", std::process::id()))
            .join("config.json")
    }

    #[test]
    fn defaults_match_frozen_schema() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert!(config.enabled);
        assert_eq!(config.gesture.finger_count, 3);
        assert_eq!(config.gesture.action, "middle_click");
        assert_eq!(config.gesture.max_duration_ms, 400);
        assert_eq!(config.gesture.max_movement, 0.08);
        assert_eq!(config.gesture.cooldown_ms, 250);
        assert!(!config.launch_at_login);
    }

    #[test]
    fn round_trips_through_disk() {
        let path = temp_file();
        let mut config = Config::default();
        config.enabled = false;
        config.gesture.finger_count = 4;
        config.gesture.max_duration_ms = 620;
        config.gesture.max_movement = 0.123;
        config.gesture.cooldown_ms = 900;

        save(&path, &config).expect("save");
        let (loaded, warnings) = load(&path);
        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert_eq!(loaded, config);

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new(".")));
    }

    #[test]
    fn missing_file_yields_defaults_without_warnings() {
        let path = temp_file();
        let (config, warnings) = load(&path);
        assert_eq!(config, Config::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn corrupt_file_yields_defaults_with_one_warning() {
        let path = temp_file();
        std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).expect("dir");
        std::fs::write(&path, b"{ this is not json").expect("write");
        let (config, warnings) = load(&path);
        assert_eq!(config, Config::default());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("could not parse"));

        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new(".")));
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let raw = r#"{
            "version": 7,
            "enabled": true,
            "gesture": {
                "finger_count": 9,
                "action": "double_click",
                "max_duration_ms": 5,
                "max_movement": 4.0,
                "cooldown_ms": 90000
            },
            "launch_at_login": true
        }"#;
        let mut config: Config = serde_json::from_str(raw).expect("parse");
        let warnings = config.sanitize();

        assert_eq!(config.version, 1);
        assert_eq!(config.gesture.finger_count, 3);
        assert_eq!(config.gesture.action, "middle_click");
        assert_eq!(config.gesture.max_duration_ms, DURATION_MIN_MS);
        assert_eq!(config.gesture.max_movement, MOVEMENT_MAX);
        assert_eq!(config.gesture.cooldown_ms, COOLDOWN_MAX_MS);
        assert!(config.launch_at_login);
        assert_eq!(warnings.len(), 6);
    }

    #[test]
    fn negative_durations_clamp_instead_of_failing_the_document() {
        let raw = r#"{"gesture":{"max_duration_ms":-20,"cooldown_ms":-5}}"#;
        let mut config: Config = serde_json::from_str(raw).expect("parse");
        config.sanitize();
        assert_eq!(config.gesture.max_duration_ms, DURATION_MIN_MS);
        assert_eq!(config.gesture.cooldown_ms, COOLDOWN_MIN_MS);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let raw = r#"{"wat": 1, "gesture": {"nope": true, "finger_count": 3}}"#;
        let config: Config = serde_json::from_str(raw).expect("parse");
        assert_eq!(config.gesture.finger_count, 3);
    }

    #[test]
    fn partial_document_uses_field_defaults() {
        let raw = r#"{"enabled": false}"#;
        let config: Config = serde_json::from_str(raw).expect("parse");
        assert!(!config.enabled);
        assert_eq!(config.gesture, GestureConfig::default());
        assert_eq!(config.version, CONFIG_VERSION);
    }

    #[test]
    fn save_creates_parent_directories() {
        let path = temp_file();
        let config = Config::default();
        save(&path, &config).expect("save");
        assert!(path.exists());
        assert!(!tmp_path(&path).exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(Path::new(".")));
    }

    #[test]
    fn params_projection_round_trips() {
        let config = Config::default();
        let params = config.params();
        assert_eq!(params.finger_count, 3);
        assert_eq!(params.max_duration_ms, 400);
        assert!((params.max_movement - 0.08).abs() < f32::EPSILON);
        assert_eq!(params.cooldown_ms, 250);

        let back = GestureConfig::from_params(params);
        assert_eq!(back, config.gesture);
    }
}
