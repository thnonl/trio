//! Tauri commands: the whole invoke surface. Small, stable, no platform types.

use tauri::{AppHandle, State};

use crate::config::{self, Config};
use crate::{lock, platform, start_engine, AppState, Status};

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.config_snapshot()
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Status {
    state.status_now()
}

#[tauri::command]
pub fn set_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Status, String> {
    let mut config = state.config_snapshot();
    if config.enabled == enabled {
        return Ok(state.status_now());
    }

    config.enabled = enabled;
    log_warnings(config.sanitize());
    config::save(&state.config_path, &config)?;

    *lock(&state.config) = config.clone();
    *lock(&state.params) = config.params();

    if enabled {
        start_engine(&app, &state.params);
    } else {
        platform::stop();
        {
            let mut engine = lock(&state.engine);
            engine.ready = false;
            engine.listener_error = None;
        }
        crate::emit_status(&app);
    }

    Ok(state.status_now())
}

#[tauri::command]
pub fn set_gesture_param(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: f64,
) -> Result<Config, String> {
    if !value.is_finite() {
        return Err(format!("value for {key} must be a finite number"));
    }

    let mut config = state.config_snapshot();
    match key.as_str() {
        "finger_count" => config.gesture.finger_count = value.round() as i64,
        "max_duration_ms" => config.gesture.max_duration_ms = value.round() as i64,
        "max_movement" => config.gesture.max_movement = value as f32,
        "cooldown_ms" => config.gesture.cooldown_ms = value.round() as i64,
        other => return Err(format!("unknown gesture parameter: {other}")),
    }

    commit(&app, &state, config)
}

/// Spec-compatible alias: both slider values in one atomic save.
#[tauri::command]
pub fn set_gesture_params(
    app: AppHandle,
    state: State<'_, AppState>,
    max_duration_ms: f64,
    max_movement: f64,
) -> Result<Config, String> {
    if !max_duration_ms.is_finite() || !max_movement.is_finite() {
        return Err("gesture parameters must be finite numbers".to_string());
    }

    let mut config = state.config_snapshot();
    config.gesture.max_duration_ms = max_duration_ms.round() as i64;
    config.gesture.max_movement = max_movement as f32;

    commit(&app, &state, config)
}

/// Spec-compatible alias: reset only the two exposed sliders.
#[tauri::command]
pub fn reset_gesture_params(app: AppHandle, state: State<'_, AppState>) -> Result<Config, String> {
    let mut config = state.config_snapshot();
    config.gesture.max_duration_ms = 400;
    config.gesture.max_movement = 0.08;

    commit(&app, &state, config)
}

#[tauri::command]
pub fn set_action(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
) -> Result<Config, String> {
    if action != config::ACTION_MIDDLE_CLICK {
        return Err(format!(
            "unsupported action {action:?}; only \"{}\" exists in this version",
            config::ACTION_MIDDLE_CLICK
        ));
    }

    let mut config = state.config_snapshot();
    config.gesture.action = action;
    config::save(&state.config_path, &config)?;

    *lock(&state.config) = config.clone();
    crate::emit_status(&app);
    Ok(config)
}

#[tauri::command]
pub fn restart_listener(app: AppHandle, state: State<'_, AppState>) -> Status {
    let enabled = state.config_snapshot().enabled;

    platform::stop();
    // Do not clear the error here: only a confirmed start may do that.
    lock(&state.engine).ready = false;

    if enabled {
        start_engine(&app, &state.params);
    } else {
        crate::emit_status(&app);
    }

    state.status_now()
}

/// Spec-compatible alias for `restart_listener`.
#[tauri::command]
pub fn retry_listener(app: AppHandle, state: State<'_, AppState>) -> Status {
    restart_listener(app, state)
}

#[tauri::command]
pub fn open_accessibility_settings() -> Result<(), String> {
    platform::open_settings()
}

/// Opens the Accessibility pane and reports the current status. The OS prompt
/// itself is the Settings pane; no C API prompt is called.
#[tauri::command]
pub fn request_permission(_app: AppHandle, state: State<'_, AppState>) -> Result<Status, String> {
    platform::open_settings()?;
    Ok(state.status_now())
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

fn commit(app: &AppHandle, state: &AppState, mut config: Config) -> Result<Config, String> {
    // Clamp to the documented ranges instead of trusting the UI.
    log_warnings(config.sanitize());
    config::save(&state.config_path, &config)?;

    *lock(&state.config) = config.clone();
    *lock(&state.params) = config.params();

    crate::emit_status(app);
    Ok(config)
}

fn log_warnings(warnings: Vec<String>) {
    for warning in warnings {
        eprintln!("[trio] config: {warning}");
    }
}
