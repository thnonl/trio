mod commands;
mod config;
// Stride-independent contact parsing: pure, compiled on every target. Its only
// non-test consumer is the macOS engine, so silence dead-code on other targets.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod contact;
// The stub platform never feeds frames, so the whole machine is "unused" on
// non-macOS builds; its unit tests still exercise every rule.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod gesture;
mod platform;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// Everything the commands and the engine need, shared through Tauri state.
pub struct AppState {
    pub config_path: PathBuf,
    pub config: Mutex<config::Config>,
    pub params: Arc<Mutex<gesture::Params>>,
    pub engine: Mutex<EngineState>,
}

#[derive(Default)]
pub struct EngineState {
    pub listener_error: Option<String>,
    /// True only after the spawned thread confirmed it reached the pumping state.
    pub ready: bool,
}

/// Snapshot handed to the UI and echoed on `trio:status` (spec §6.5).
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub enabled: bool,
    /// `"granted"`, `"missing"` or `"unsupported"`.
    pub permission: String,
    /// `"running"`, `"stopped"` or `"error"`.
    pub listener: String,
    pub listener_error: Option<String>,
}

/// Lock helper: a poisoned mutex must never take the UI down.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AppState {
    pub fn config_snapshot(&self) -> config::Config {
        lock(&self.config).clone()
    }

    pub fn status_now(&self) -> Status {
        let config = self.config_snapshot();
        let (error, ready) = {
            let engine = lock(&self.engine);
            (engine.listener_error.clone(), engine.ready)
        };
        let active = ready && platform::is_running();

        let (listener, listener_error) = match (error, active) {
            (Some(reason), _) => ("error", Some(reason)),
            (None, true) => ("running", None),
            (None, false) => ("stopped", None),
        };

        Status {
            enabled: config.enabled,
            permission: platform::permission().as_str().to_string(),
            listener: listener.to_string(),
            listener_error,
        }
    }
}

pub fn emit_status(app: &AppHandle) {
    let status = app.state::<AppState>().status_now();
    if let Err(err) = app.emit("trio:status", &status) {
        eprintln!("[trio] could not emit trio:status: {err}");
    }
}

/// Start the listener. The status may only become "running" after the platform
/// confirms the thread reached its pumping state; failures — including ones that
/// happen right after spawning — are stored and surfaced as the listener banner.
pub(crate) fn start_engine(app: &AppHandle, params: &Arc<Mutex<gesture::Params>>) {
    let sink: platform::EventSink = {
        let app = app.clone();
        Arc::new(move |event| match event {
            platform::Event::Tap => {}
            platform::Event::Error(reason) => {
                eprintln!("[trio] listener: {reason}");
                {
                    let state = app.state::<AppState>();
                    let mut engine = lock(&state.engine);
                    engine.ready = false;
                    engine.listener_error = Some(reason);
                }
                emit_status(&app);
            }
        })
    };

    // A new attempt is in flight. Keep any previous error until the outcome is
    // known; clearing it here is exactly the bug that hid dead listeners.
    {
        let state = app.state::<AppState>();
        lock(&state.engine).ready = false;
    }

    let handle = match platform::start(params.clone(), sink) {
        Ok(handle) => handle,
        Err(reason) => {
            eprintln!("[trio] listener unavailable: {reason}");
            finish_listener_attempt(app, false, Some(reason));
            return;
        }
    };

    match handle.wait_ready(Duration::from_millis(150)) {
        Ok(()) => finish_listener_attempt(app, true, None),
        Err(reason) => {
            eprintln!("[trio] listener unavailable: {reason}");
            finish_listener_attempt(app, false, Some(reason));
        }
    }
}

fn finish_listener_attempt(app: &AppHandle, ready: bool, error: Option<String>) {
    {
        let state = app.state::<AppState>();
        let mut engine = lock(&state.engine);
        engine.ready = ready;
        engine.listener_error = error;
    }
    emit_status(app);
}

/// Poll the Accessibility permission so granting it in System Settings updates
/// the window without a restart. Cheap enough (one C call every two seconds).
fn spawn_permission_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = platform::permission();
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let now = platform::permission();
            if now != last {
                last = now;
                emit_status(&app);
            }
        }
    });
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = match app.path().app_config_dir() {
                Ok(dir) => dir,
                Err(err) => {
                    eprintln!("[trio] could not resolve the config directory ({err}), using temp");
                    std::env::temp_dir().join("trio")
                }
            };
            let config_path = config_dir.join("config.json");

            let (config, warnings) = config::load(&config_path);
            for warning in &warnings {
                eprintln!("[trio] config: {warning}");
            }

            if !config_path.exists() {
                eprintln!(
                    "[trio] no config at {}, using defaults",
                    config_path.display()
                );
                if let Err(err) = config::save(&config_path, &config) {
                    eprintln!("[trio] could not write default config: {err}");
                }
            }

            let params = Arc::new(Mutex::new(config.params()));

            app.manage(AppState {
                config_path,
                config: Mutex::new(config.clone()),
                params: params.clone(),
                engine: Mutex::new(EngineState::default()),
            });

            // The stub reports the truth on non-macOS; do not auto-start there.
            #[cfg(target_os = "macos")]
            if config.enabled {
                start_engine(app.handle(), &params);
            }

            spawn_permission_watch(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::get_status,
            commands::set_enabled,
            commands::set_gesture_param,
            commands::set_gesture_params,
            commands::reset_gesture_params,
            commands::set_action,
            commands::open_accessibility_settings,
            commands::request_permission,
            commands::restart_listener,
            commands::retry_listener,
            commands::quit,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Lead ruling: closing hides the window; Quit / ⌘Q is the only
                // exit path (the macOS default menu supplies it).
                api.prevent_close();
                if let Err(err) = window.hide() {
                    eprintln!("[trio] could not hide the window: {err}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::get_status,
            commands::set_enabled,
            commands::set_gesture_param,
            commands::set_gesture_params,
            commands::reset_gesture_params,
            commands::set_action,
            commands::open_accessibility_settings,
            commands::request_permission,
            commands::restart_listener,
            commands::retry_listener,
            commands::quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Trio")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            handle_run_event(app, event);

            #[cfg(not(target_os = "macos"))]
            {
                let _ = (app, event);
            }
        });
}

/// macOS: a Dock click with no visible window re-shows the main window.
#[cfg(target_os = "macos")]
fn handle_run_event(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Reopen {
        has_visible_windows,
    } = event
    {
        if !has_visible_windows {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}
