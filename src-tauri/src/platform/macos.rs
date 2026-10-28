//! macOS multitouch engine.
//!
//! Risk-reduction choice (charter + Lead): this crate links **nothing** from
//! macOS frameworks. Every symbol is resolved at runtime with `dlopen`/`dlsym`,
//! so a missing/renamed private symbol becomes a readable error in the UI
//! instead of a link-time or launch-time crash.
//!
//! Only `dlopen`, `dlsym` and `dlerror` come from libSystem, which every macOS
//! process links anyway; it cannot fail to resolve.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::contact;
use crate::gesture::{self, Contact, Params};

use super::{Event, EventSink, ListenerHandle, Permission};

// ---------------------------------------------------------------------------
// libSystem entry points (always present on macOS)
// ---------------------------------------------------------------------------

#[link(name = "System")]
extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}

const RTLD_NOW: c_int = 2;

const MT_FRAMEWORK: &str =
    "/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport";
const CF_FRAMEWORK: &str = "/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation";
const CG_FRAMEWORK: &str = "/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics";
const AX_FRAMEWORK: &str =
    "/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices";

/// `kCFStringEncodingUTF8`
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCFRunLoopDefaultMode` is a CFString whose value is its own name.
const RUN_LOOP_DEFAULT_MODE: &str = "kCFRunLoopDefaultMode";

// CoreGraphics event constants.
const CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
const CG_MOUSE_BUTTON_CENTER: u32 = 2;
const CG_HID_EVENT_TAP: u32 = 0;

/// Runtime contact stride.
///
/// 80 is the widely published `MTContact` size; 88 is the common
/// reverse-engineered variant. The record is never modelled as a struct, only
/// read through `contact::parse`, so the stride is a number, not an ABI.
/// Override for experiments: `TRIO_CONTACT_STRIDE=80|88|96`.
fn contact_stride() -> usize {
    static STRIDE: OnceLock<usize> = OnceLock::new();
    *STRIDE.get_or_init(
        || match std::env::var("TRIO_CONTACT_STRIDE").ok().as_deref() {
            Some("80") => 80,
            Some("96") => 96,
            _ => 88,
        },
    )
}

// ---------------------------------------------------------------------------
// Runtime linked types
// ---------------------------------------------------------------------------

type CfArrayRef = *const c_void;
type MtDeviceRef = *mut c_void;

/// Contact frame callback, exactly the signature MultitouchSupport uses.
type MtContactCallback = extern "C" fn(*mut c_void, *mut MtContact, c_int, f64, c_int);

/// Opaque contact record.
///
/// The true layout differs between macOS builds (80/88/96-byte strides), so this
/// type is never read field-by-field. `contact::parse` reads the normalized
/// position at byte offset 24 using the runtime stride; nothing here asserts a
/// size that only proves our own definition.
#[repr(C)]
#[allow(dead_code)]
pub struct MtContact {
    _opaque: [u8; 0],
}

/// CoreGraphics point (`CGFloat` is `f64` on every supported macOS).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct CGPoint {
    x: f64,
    y: f64,
}

// ---------------------------------------------------------------------------
// Symbol tables
// ---------------------------------------------------------------------------

struct MtSymbols {
    device_create_list: unsafe extern "C" fn() -> CfArrayRef,
    register_contact_frame_callback: unsafe extern "C" fn(MtDeviceRef, MtContactCallback),
    device_start: unsafe extern "C" fn(MtDeviceRef, c_int) -> c_int,
    #[allow(dead_code)] // devices live for the process lifetime
    device_release: unsafe extern "C" fn(MtDeviceRef),
}

struct CfSymbols {
    array_get_count: unsafe extern "C" fn(CfArrayRef) -> isize,
    array_get_value_at_index: unsafe extern "C" fn(CfArrayRef, isize) -> *const c_void,
    /// Optional: if CoreFoundation lacks it we fall back to a sleep loop.
    run_loop_run_in_mode: Option<unsafe extern "C" fn(*const c_void, f64, u8) -> i32>,
    release: unsafe extern "C" fn(*const c_void),
}

struct CgSymbols {
    event_create: unsafe extern "C" fn(*const c_void) -> *const c_void,
    event_get_location: unsafe extern "C" fn(*const c_void) -> CGPoint,
    create_mouse_event: unsafe extern "C" fn(*const c_void, u32, CGPoint, u32) -> *const c_void,
    event_post: unsafe extern "C" fn(u32, *const c_void),
    release: unsafe extern "C" fn(*const c_void),
}

static MT_SYMBOLS: OnceLock<Result<MtSymbols, String>> = OnceLock::new();
static CF_SYMBOLS: OnceLock<Result<CfSymbols, String>> = OnceLock::new();
static CG_SYMBOLS: OnceLock<Result<CgSymbols, String>> = OnceLock::new();
static AX_SYMBOLS: OnceLock<Result<unsafe extern "C" fn() -> u8, String>> = OnceLock::new();

fn dlerror_string() -> String {
    let raw = unsafe { dlerror() };
    if raw.is_null() {
        "unknown dlopen/dlsym error".to_string()
    } else {
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned()
    }
}

fn open_framework(path: &str) -> Result<*mut c_void, String> {
    let name = CString::new(path).map_err(|_| format!("invalid framework path: {path}"))?;
    let handle = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
    if handle.is_null() {
        Err(format!("{path} (dlopen failed: {})", dlerror_string()))
    } else {
        Ok(handle)
    }
}

fn lookup(handle: *mut c_void, framework: &str, symbol: &str) -> Result<*mut c_void, String> {
    // Clear any stale error so a NULL result is unambiguous.
    unsafe {
        let _ = dlerror();
    }
    let name = CString::new(symbol).map_err(|_| format!("invalid symbol name: {symbol}"))?;
    let raw = unsafe { dlsym(handle, name.as_ptr()) };
    if raw.is_null() {
        Err(format!("{framework}: symbol {symbol} not found"))
    } else {
        Ok(raw)
    }
}

macro_rules! resolve {
    ($handle:expr, $framework:expr, $symbol:literal, $ty:ty) => {{
        let raw = lookup($handle, $framework, $symbol)?;
        // SAFETY: the pointer came from dlsym for exactly this symbol and the
        // declared signature matches the C API.
        unsafe { std::mem::transmute::<*mut c_void, $ty>(raw) }
    }};
}

/// Like `resolve!` but returns `Result` so callers can map the error to their
/// own user-facing message.
macro_rules! try_resolve {
    ($handle:expr, $framework:expr, $symbol:literal, $ty:ty) => {{
        lookup($handle, $framework, $symbol)
            .map(|raw| unsafe { std::mem::transmute::<*mut c_void, $ty>(raw) })
    }};
}

fn resolve_mt() -> Result<MtSymbols, String> {
    let path = mt_framework_path();
    let handle = match open_framework(&path) {
        Ok(handle) => handle,
        Err(err) => {
            // Full detail on stderr; the UI shows the spec's exact string.
            eprintln!("[trio] {err}");
            return Err("MultitouchSupport could not be loaded.".to_string());
        }
    };

    Ok(MtSymbols {
        device_create_list: try_resolve!(
            handle,
            path.as_str(),
            "MTDeviceCreateList",
            unsafe extern "C" fn() -> CfArrayRef
        )
        .map_err(missing_symbol)?,
        register_contact_frame_callback: try_resolve!(
            handle,
            path.as_str(),
            "MTRegisterContactFrameCallback",
            unsafe extern "C" fn(MtDeviceRef, MtContactCallback)
        )
        .map_err(missing_symbol)?,
        device_start: try_resolve!(
            handle,
            path.as_str(),
            "MTDeviceStart",
            unsafe extern "C" fn(MtDeviceRef, c_int) -> c_int
        )
        .map_err(missing_symbol)?,
        device_release: try_resolve!(
            handle,
            path.as_str(),
            "MTDeviceRelease",
            unsafe extern "C" fn(MtDeviceRef)
        )
        .map_err(missing_symbol)?,
    })
}

/// Spec copy for a framework that exists but lacks a required symbol.
fn missing_symbol(err: String) -> String {
    eprintln!("[trio] {err}");
    "MultitouchSupport is missing required functions.".to_string()
}

/// `TRIO_MT_PATH` overrides the framework path (useful to prove the error path
/// and to test a different macOS build). Defaults to the charter path.
fn mt_framework_path() -> String {
    match std::env::var("TRIO_MT_PATH") {
        Ok(path) if !path.is_empty() => path,
        _ => MT_FRAMEWORK.to_string(),
    }
}

fn resolve_cf() -> Result<CfSymbols, String> {
    let handle = open_framework(CF_FRAMEWORK)?;
    Ok(CfSymbols {
        array_get_count: resolve!(
            handle,
            CF_FRAMEWORK,
            "CFArrayGetCount",
            unsafe extern "C" fn(CfArrayRef) -> isize
        ),
        array_get_value_at_index: resolve!(
            handle,
            CF_FRAMEWORK,
            "CFArrayGetValueAtIndex",
            unsafe extern "C" fn(CfArrayRef, isize) -> *const c_void
        ),
        run_loop_run_in_mode: lookup(handle, CF_FRAMEWORK, "CFRunLoopRunInMode").ok().map(
            |raw| unsafe {
                std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(*const c_void, f64, u8) -> i32,
                >(raw)
            },
        ),
        release: resolve!(
            handle,
            CF_FRAMEWORK,
            "CFRelease",
            unsafe extern "C" fn(*const c_void)
        ),
    })
}

fn resolve_cg() -> Result<CgSymbols, String> {
    let handle = open_framework(CG_FRAMEWORK)?;
    Ok(CgSymbols {
        event_create: resolve!(
            handle,
            CG_FRAMEWORK,
            "CGEventCreate",
            unsafe extern "C" fn(*const c_void) -> *const c_void
        ),
        event_get_location: resolve!(
            handle,
            CG_FRAMEWORK,
            "CGEventGetLocation",
            unsafe extern "C" fn(*const c_void) -> CGPoint
        ),
        create_mouse_event: resolve!(
            handle,
            CG_FRAMEWORK,
            "CGEventCreateMouseEvent",
            unsafe extern "C" fn(*const c_void, u32, CGPoint, u32) -> *const c_void
        ),
        event_post: resolve!(
            handle,
            CG_FRAMEWORK,
            "CGEventPost",
            unsafe extern "C" fn(u32, *const c_void)
        ),
        release: resolve!(
            handle,
            CG_FRAMEWORK,
            "CFRelease",
            unsafe extern "C" fn(*const c_void)
        ),
    })
}

fn resolve_ax() -> Result<unsafe extern "C" fn() -> u8, String> {
    let handle = open_framework(AX_FRAMEWORK)?;
    Ok(resolve!(
        handle,
        AX_FRAMEWORK,
        "AXIsProcessTrusted",
        unsafe extern "C" fn() -> u8
    ))
}

fn mt_symbols() -> Result<&'static MtSymbols, String> {
    match MT_SYMBOLS.get_or_init(resolve_mt) {
        Ok(symbols) => Ok(symbols),
        Err(err) => Err(err.clone()),
    }
}

fn cf_symbols() -> Result<&'static CfSymbols, String> {
    match CF_SYMBOLS.get_or_init(resolve_cf) {
        Ok(symbols) => Ok(symbols),
        Err(err) => Err(err.clone()),
    }
}

fn cg_symbols() -> Result<&'static CgSymbols, String> {
    match CG_SYMBOLS.get_or_init(resolve_cg) {
        Ok(symbols) => Ok(symbols),
        Err(err) => Err(err.clone()),
    }
}

// ---------------------------------------------------------------------------
// Accessibility permission
// ---------------------------------------------------------------------------

/// `AXIsProcessTrusted() != 0`. Never panics; an unresolvable symbol reads as
/// "missing" and logs once.
pub fn permission() -> Permission {
    static LOGGED: AtomicBool = AtomicBool::new(false);

    match AX_SYMBOLS.get_or_init(resolve_ax) {
        Ok(check) => {
            if unsafe { check() != 0 } {
                Permission::Granted
            } else {
                Permission::Missing
            }
        }
        Err(err) => {
            if !LOGGED.swap(true, Ordering::SeqCst) {
                eprintln!("[trio] accessibility check unavailable: {err}");
            }
            Permission::Missing
        }
    }
}

/// The permission prompt is not driven through the C API: open the exact
/// Privacy > Accessibility pane and let the user flip the switch.
pub fn open_settings() -> Result<(), String> {
    const URL: &str =
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

    let status = std::process::Command::new("/usr/bin/open")
        .arg(URL)
        .status()
        .map_err(|err| format!("could not launch /usr/bin/open: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("/usr/bin/open exited with {status}"))
    }
}

// ---------------------------------------------------------------------------
// Middle click
// ---------------------------------------------------------------------------

fn click_middle() -> Result<(), String> {
    let cg = cg_symbols()?;

    // SAFETY: every pointer is NULL-checked before use; CGEvent objects are
    // released exactly once.
    unsafe {
        let source = (cg.event_create)(ptr::null());
        if source.is_null() {
            return Err("CGEventCreate returned NULL".to_string());
        }
        let location = (cg.event_get_location)(source);
        (cg.release)(source);

        let down = (cg.create_mouse_event)(
            ptr::null(),
            CG_EVENT_OTHER_MOUSE_DOWN,
            location,
            CG_MOUSE_BUTTON_CENTER,
        );
        if down.is_null() {
            return Err("CGEventCreateMouseEvent (down) returned NULL".to_string());
        }
        (cg.event_post)(CG_HID_EVENT_TAP, down);
        (cg.release)(down);

        std::thread::sleep(Duration::from_millis(10));

        let up = (cg.create_mouse_event)(
            ptr::null(),
            CG_EVENT_OTHER_MOUSE_UP,
            location,
            CG_MOUSE_BUTTON_CENTER,
        );
        if up.is_null() {
            return Err("CGEventCreateMouseEvent (up) returned NULL".to_string());
        }
        (cg.event_post)(CG_HID_EVENT_TAP, up);
        (cg.release)(up);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Listener thread
// ---------------------------------------------------------------------------

static RUNNING: AtomicBool = AtomicBool::new(false);
static CTX: AtomicPtr<Ctx> = AtomicPtr::new(ptr::null_mut());
/// Bumped on every stop/start. A listener thread only owns the engine while the
/// generation it was spawned with is still current.
static GENERATION: AtomicU64 = AtomicU64::new(1);
static ACTIVE_GENERATION: AtomicU64 = AtomicU64::new(0);

struct Ctx {
    epoch: Instant,
    params: Arc<Mutex<Params>>,
    machine: Mutex<gesture::Machine>,
    sink: EventSink,
}

/// Start the engine. Fails fast (and synchronously) when the private framework
/// or a required symbol is missing. On success it returns a handle the caller
/// must `wait_ready` on: the thread reports once the devices are registered, so
/// a thread that dies right after spawning can never look healthy.
pub fn start(params: Arc<Mutex<Params>>, sink: EventSink) -> Result<ListenerHandle, String> {
    if is_running() {
        return Ok(ListenerHandle::already_running());
    }

    mt_symbols()?;
    cf_symbols()?;

    let initial = params.lock().map(|p| *p).unwrap_or_default();
    let ctx = Box::new(Ctx {
        epoch: Instant::now(),
        params,
        machine: Mutex::new(gesture::Machine::new(initial)),
        sink,
    });
    let address = Box::into_raw(ctx) as usize;
    let generation = GENERATION.load(Ordering::SeqCst);

    let (ready_tx, ready_rx) = mpsc::channel();
    let alive = Arc::new(AtomicBool::new(true));
    let last_error = Arc::new(Mutex::new(None));

    CTX.store(address as *mut Ctx, Ordering::SeqCst);
    ACTIVE_GENERATION.store(generation, Ordering::SeqCst);
    RUNNING.store(true, Ordering::SeqCst);

    let thread_alive = Arc::clone(&alive);
    let thread_error = Arc::clone(&last_error);

    let spawned = std::thread::Builder::new()
        .name("trio-multitouch".to_string())
        .spawn(move || {
            // SAFETY: `address` points at a leaked Box<Ctx>; stop() only clears
            // the CTX slot, it never frees, so a late device callback can never
            // observe freed memory.
            let ctx = unsafe { &*(address as *const Ctx) };

            if let Err(err) = run_device_loop(generation, &ready_tx) {
                if let Ok(mut slot) = thread_error.lock() {
                    *slot = Some(err.clone());
                }
                let _ = ready_tx.send(Err(err.clone()));
                (ctx.sink)(Event::Error(err));
            }

            thread_alive.store(false, Ordering::SeqCst);

            // Only the current generation may publish "stopped"; a superseded
            // thread must not clobber the listener that replaced it.
            if ACTIVE_GENERATION.load(Ordering::SeqCst) == generation {
                RUNNING.store(false, Ordering::SeqCst);
                CTX.store(ptr::null_mut(), Ordering::SeqCst);
            }
        });

    if let Err(err) = spawned {
        alive.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
        ACTIVE_GENERATION.store(0, Ordering::SeqCst);
        CTX.store(ptr::null_mut(), Ordering::SeqCst);
        return Err(format!("could not spawn listener thread: {err}"));
    }

    Ok(ListenerHandle::new(ready_rx, alive, last_error))
}

/// Stop feeding events and invalidate any in-flight listener thread. Device
/// callbacks stay registered for the process lifetime (the private API has no
/// matching unregister); they become no-ops because the context pointer is
/// cleared and their generation is stale.
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    CTX.store(ptr::null_mut(), Ordering::SeqCst);
    RUNNING.store(false, Ordering::SeqCst);
}

pub fn is_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
        && ACTIVE_GENERATION.load(Ordering::SeqCst) == GENERATION.load(Ordering::SeqCst)
}

fn run_device_loop(generation: u64, ready: &Sender<Result<(), String>>) -> Result<(), String> {
    let mt = mt_symbols()?;
    let cf = cf_symbols()?;

    // SAFETY: calls follow the private API contract; every pointer checked.
    unsafe {
        let list = (mt.device_create_list)();
        if list.is_null() {
            return Err("No trackpad detected.".to_string());
        }

        let count = (cf.array_get_count)(list);
        if count <= 0 {
            (cf.release)(list);
            return Err("No trackpad detected.".to_string());
        }

        let mut registered = 0usize;
        for index in 0..count {
            let device = (cf.array_get_value_at_index)(list, index) as MtDeviceRef;
            if device.is_null() {
                continue;
            }
            (mt.register_contact_frame_callback)(device, on_contacts);
            let _ = (mt.device_start)(device, 0);
            registered += 1;
        }
        (cf.release)(list);

        if registered == 0 {
            return Err("No trackpad detected.".to_string());
        }

        // Confirm only once the devices are registered: that is the first moment
        // the listener can actually deliver frames.
        match (cf.run_loop_run_in_mode, default_run_loop_mode()) {
            (Some(run), Ok(mode)) => {
                let _ = ready.send(Ok(()));

                // Blocking pump. `returnAfterSourceHandled = false` means a
                // contact callback does NOT make the call return, so this cannot
                // spin (AC-29). The large interval parks the thread; shutdown is
                // the generation flag + CTX clearing. An old thread may stay in
                // the run loop after stop() (no unregister API), which is why
                // callbacks re-read CTX before touching any state.
                while GENERATION.load(Ordering::SeqCst) == generation {
                    (run)(mode, 1.0e10, 0);
                }
                (cf.release)(mode);
            }
            _ => {
                // No run loop at all: stay alive for callback delivery without
                // burning CPU, and keep checking the shutdown flag.
                eprintln!("[trio] CFRunLoop unavailable; using the sleep fallback");
                let _ = ready.send(Ok(()));
                while GENERATION.load(Ordering::SeqCst) == generation {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    Ok(())
}

/// The default run loop mode as a CFString.
///
/// Preferred path: build the string with `CFStringCreateWithCString`.
/// Fallback: dereference the exported `kCFRunLoopDefaultMode` data symbol.
/// We never pass NULL: a NULL mode would be a hard crash inside CoreFoundation,
/// so if both paths fail we return a readable error instead (documented choice).
fn default_run_loop_mode() -> Result<*const c_void, String> {
    let handle = open_framework(CF_FRAMEWORK)?;

    if let Ok(raw) = lookup(handle, CF_FRAMEWORK, "CFStringCreateWithCString") {
        // SAFETY: dlsym returned this exact symbol; signature matches.
        let create: unsafe extern "C" fn(*const c_void, *const c_char, u32) -> *const c_void = unsafe {
            std::mem::transmute::<
                *mut c_void,
                unsafe extern "C" fn(*const c_void, *const c_char, u32) -> *const c_void,
            >(raw)
        };
        let name = CString::new(RUN_LOOP_DEFAULT_MODE)
            .map_err(|_| "invalid run loop mode name".to_string())?;
        let mode = unsafe { create(ptr::null(), name.as_ptr(), CF_STRING_ENCODING_UTF8) };
        if !mode.is_null() {
            return Ok(mode);
        }
    }

    if let Ok(raw) = lookup(handle, CF_FRAMEWORK, RUN_LOOP_DEFAULT_MODE) {
        // The data symbol is a `CFStringRef` variable: dereference it once.
        let mode = unsafe { *(raw as *const *const c_void) };
        if !mode.is_null() {
            return Ok(mode);
        }
    }

    Err("could not resolve the default CFRunLoop mode".to_string())
}

/// Contact frame callback. Runs on the listener thread's run loop.
extern "C" fn on_contacts(
    _device: *mut c_void,
    contacts: *mut MtContact,
    count: c_int,
    _timestamp: f64,
    _frame: c_int,
) {
    let raw = CTX.load(Ordering::SeqCst);
    if raw.is_null() {
        return;
    }

    // SAFETY: raw is a leaked Box<Ctx> (see start()).
    let ctx = unsafe { &*raw };

    if contacts.is_null() || count <= 0 {
        // An empty frame is what ends a tap candidate.
        feed(ctx, &[]);
        return;
    }

    // A real frame holds a handful of contacts; bound the read defensively.
    let count = count.min(64);
    let positions = contact::parse(contacts as *const u8, count, contact_stride());

    if debug_enabled() {
        eprintln!(
            "[trio] frame contacts={count} stride={} positions={:?}",
            contact_stride(),
            positions
        );
    }

    let touched: Vec<Contact> = positions
        .iter()
        .map(|position| Contact::new(position.x, position.y))
        .collect();

    feed(ctx, &touched);
}

/// `TRIO_DEBUG=1` logs every parsed contact frame to stderr so the first Mac
/// tester can confirm the stride and position offset without a debugger.
fn debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| matches!(std::env::var("TRIO_DEBUG"), Ok(value) if value == "1"))
}

fn feed(ctx: &Ctx, contacts: &[Contact]) {
    let params = match ctx.params.lock() {
        Ok(params) => *params,
        Err(poisoned) => *poisoned.into_inner(),
    };
    let now_ms = ctx.epoch.elapsed().as_millis() as u64;

    let outcome = match ctx.machine.lock() {
        Ok(mut machine) => {
            machine.set_params(params);
            machine.push_frame(gesture::Frame::new(now_ms, contacts))
        }
        Err(poisoned) => {
            let mut machine = poisoned.into_inner();
            machine.set_params(params);
            machine.push_frame(gesture::Frame::new(now_ms, contacts))
        }
    };

    if outcome == gesture::Outcome::MiddleClick {
        match click_middle() {
            Ok(()) => (ctx.sink)(Event::Tap),
            Err(err) => (ctx.sink)(Event::Error(format!("middle click failed: {err}"))),
        }
    }
}
