//! Non-macOS stub with the same interface as `macos.rs`.
//!
//! Windows/Linux builds only need `cargo check` and the unit tests, so the
//! engine reports itself as unavailable and does nothing else.

use std::sync::{Arc, Mutex};

use crate::gesture::Params;

use super::{EventSink, ListenerHandle, Permission};

pub fn permission() -> Permission {
    Permission::Unsupported
}

pub fn open_settings() -> Result<(), String> {
    Err("Accessibility settings are only available on macOS".to_string())
}

pub fn start(_params: Arc<Mutex<Params>>, _sink: EventSink) -> Result<ListenerHandle, String> {
    Err("Gesture listener runs on macOS only.".to_string())
}

pub fn stop() {}

pub fn is_running() -> bool {
    false
}
