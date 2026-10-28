//! Platform boundary. macOS gets the real engine, every other target gets a
//! stub with the same interface so the crate (and its tests) build anywhere.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Something the engine wants the UI to know about. `Tap` is currently only
/// logged; `Error` is surfaced as the listener banner.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the macOS engine emits these
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Tap,
    Error(String),
}

/// Callback handed to the platform layer so it never needs a Tauri handle.
pub type EventSink = Arc<dyn Fn(Event) + Send + Sync + 'static>;

/// Accessibility trust, as the UI's status record reports it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // only the stub consumes Unsupported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    Granted,
    Missing,
    Unsupported,
}

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::Granted => "granted",
            Permission::Missing => "missing",
            Permission::Unsupported => "unsupported",
        }
    }
}

/// Readiness of a spawned listener thread.
///
/// `start` returns as soon as the thread exists; the thread then reports whether
/// it actually reached the pumping state (devices registered) or failed. The UI
/// must not be told "running" before that signal, otherwise a thread that dies
/// right after spawn would look healthy.
pub struct ListenerHandle {
    ready: Receiver<Result<(), String>>,
    alive: Arc<AtomicBool>,
    last_error: Arc<Mutex<Option<String>>>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // constructors are macOS-only
impl ListenerHandle {
    pub(crate) fn new(
        ready: Receiver<Result<(), String>>,
        alive: Arc<AtomicBool>,
        last_error: Arc<Mutex<Option<String>>>,
    ) -> Self {
        Self {
            ready,
            alive,
            last_error,
        }
    }

    /// Handle for an already-running listener: confirmed immediately.
    pub(crate) fn already_running() -> Self {
        let (sender, ready) = mpsc::channel();
        let _ = sender.send(Ok(()));
        Self {
            ready,
            alive: Arc::new(AtomicBool::new(true)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Wait up to `timeout` for the thread to confirm it is running.
    ///
    /// - `Ok(())`: devices registered (or the thread is still alive at the
    ///   deadline, per the fix spec; a later failure arrives as an `Event::Error`).
    /// - `Err(reason)`: the thread reported a failure, or it died without a
    ///   confirmation (the stored reason is used when available).
    pub fn wait_ready(&self, timeout: Duration) -> Result<(), String> {
        match self.ready.recv_timeout(timeout) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(reason)) => Err(reason),
            Err(RecvTimeoutError::Disconnected) => Err(self.last_reason()),
            Err(RecvTimeoutError::Timeout) => {
                if self.alive.load(Ordering::SeqCst) {
                    Ok(())
                } else {
                    Err(self.last_reason())
                }
            }
        }
    }

    fn last_reason(&self) -> String {
        self.last_error
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .unwrap_or_else(|| "Gesture listener stopped.".to_string())
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{is_running, open_settings, permission, start, stop};

#[cfg(not(target_os = "macos"))]
mod stub;
#[cfg(not(target_os = "macos"))]
pub use stub::{is_running, open_settings, permission, start, stop};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmed_thread_reports_running() {
        let (tx, rx) = mpsc::channel();
        let handle = ListenerHandle::new(
            rx,
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(None)),
        );
        tx.send(Ok(())).expect("send");
        assert_eq!(handle.wait_ready(Duration::from_millis(10)), Ok(()));
    }

    #[test]
    fn failed_thread_reports_the_reason() {
        let (tx, rx) = mpsc::channel();
        let handle = ListenerHandle::new(
            rx,
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(None)),
        );
        tx.send(Err("No trackpad detected.".to_string()))
            .expect("send");
        assert_eq!(
            handle.wait_ready(Duration::from_millis(10)),
            Err("No trackpad detected.".to_string())
        );
    }

    #[test]
    fn silent_death_reports_an_error_not_running() {
        let (_tx, rx) = mpsc::channel();
        let handle = ListenerHandle::new(
            rx,
            Arc::new(AtomicBool::new(false)),
            Arc::new(Mutex::new(Some("thread died".to_string()))),
        );
        assert_eq!(
            handle.wait_ready(Duration::from_millis(10)),
            Err("thread died".to_string())
        );
    }

    #[test]
    fn silent_but_alive_thread_is_provisional_running() {
        let (_tx, rx) = mpsc::channel();
        let handle = ListenerHandle::new(
            rx,
            Arc::new(AtomicBool::new(true)),
            Arc::new(Mutex::new(None)),
        );
        assert_eq!(handle.wait_ready(Duration::from_millis(10)), Ok(()));
    }

    #[test]
    fn already_running_is_confirmed() {
        assert_eq!(
            ListenerHandle::already_running().wait_ready(Duration::from_millis(10)),
            Ok(())
        );
    }
}
