# Charter — Trio (BTT-lite for macOS)

Project root: `E:\3touch`
Artifacts: `E:\3touch\docs\work\` (each role writes only its own files)
Owner: the human user (Vietnamese). Team works in English.

## Mission
A tiny, modern, simple macOS utility that does one thing extremely well: **3-finger tap → middle click**.
Inspired by BetterTouchTool, but scoped: config for one gesture first, extensible later.

Stack: **Rust + Tauri 2**, vanilla HTML/CSS/JS frontend (no bundler, no node build step).
Target: macOS 13+. Dev box here is **Windows**, so all macOS-only code must be `#[cfg(target_os = "macos")]`
and a stub must keep `cargo check` green on Windows.

## Non-negotiable constraints (owner)
1. 3-finger tap → middle click must work end-to-end first. Everything else is secondary.
2. Window is **not resizable**, fixed size, opens centered.
3. Design: modern, minimal, easy. No settings maze. One main card, one toggle, one action row.
4. MVP first: no BTT feature spread (no keyboard shortcuts, no per-app bindings, no window snapping).
5. Product UI language: **English** (macOS utility convention). Docs: English.

## Architecture (fixed by Lead — do not change without telling Lead)
```
E:\3touch\
  src-tauri\Cargo.toml
  src-tauri\tauri.conf.json
  src-tauri\capabilities\default.json
  src-tauri\icons\            (generated; app must still check without them)
  src-tauri\src\main.rs       entry
  src-tauri\src\lib.rs        tauri builder, window setup
  src-tauri\src\config.rs     Config serde + load/save JSON
  src-tauri\src\gesture.rs    PURE state machine, unit-tested, no platform code
  src-tauri\src\platform\mod.rs
  src-tauri\src\platform\macos.rs   MultitouchSupport (dlopen) + CGEvent + AX trust
  src-tauri\src\platform\stub.rs    non-macOS no-op
  src-tauri\src\commands.rs   tauri commands
  src\index.html  src\styles.css  src\main.js
  docs\work\...   team artifacts
  scripts\dev-mac.sh, scripts\check.ps1
```

### Config schema (v1, frozen)
```json
{
  "version": 1,
  "enabled": true,
  "gesture": {
    "finger_count": 3,
    "action": "middle_click",
    "max_duration_ms": 400,
    "max_movement": 0.08,
    "cooldown_ms": 250
  },
  "launch_at_login": false
}
```
Stored at `app_config_dir()/config.json`. Written atomically (tmp + rename). Missing file → defaults.
Unknown fields ignored on load; invalid values clamped to safe range.

### Gesture detection rules (pure, in `gesture.rs`)
Input frames: `contacts: &[Contact { x: f32, y: f32 }]`, `t: Instant` (testable via `u64 ms`).
- A tap candidate starts on the first frame with `contacts.len() >= 3`.
- Track `peak_fingers = max(len)` during the candidate.
- Candidate ends when it drops below 3 (fingers lifted).
- Emit `MiddleClick` only if **all** hold:
  - `peak_fingers == finger_count` (4 fingers must NOT fire)
  - duration `<= max_duration_ms`
  - centroid travel from start `<= max_movement` (normalized units, scroll/pinch must NOT fire)
  - `now - last_emit >= cooldown_ms` (no double fire)
- Lifting order does not matter. Resting fingers (stationary contact frames) must still fire.
- Any exception → `None`, state resets, no partial state leaks.

### macOS engine (`platform/macos.rs`)
- `dlopen`/`dlsym` on `/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport`
  for `MTDeviceCreateList`, `MTRegisterContactFrameCallback`, `MTDeviceStart`, `MTDeviceRelease`.
  **No link-time dependency** on the private framework; if dlopen fails → surface a readable error to the UI, never panic.
- Contact frame callback: `callback(device, contacts: *mut MTContact, count: i32, timestamp: f64, frame: i32)` → feed `gesture::Machine`.
- Middle click on macOS: `CGEventCreateMouseEvent`/`CGEventPost(kCGHIDEventTap)` with `kCGEventOtherMouseDown`/`Up`, button 2, at current cursor location, ~10 ms apart. Declare via the platform C API (core-graphics crate is allowed if it compiles on macOS).
- Accessibility permission: `AXIsProcessTrusted()` check + prompt variant, surfaced as a command. Never block the UI thread.
- Everything must degrade to "listener unavailable: <reason>" instead of crashing.

## Environment facts
- Dev box: Windows, repo empty at start. No macOS machine available to the team.
- Therefore: **macOS runtime behaviour cannot be verified by this team**; the gate is
  `cargo check` + `cargo test` (unit tests on the state machine) + frontend loaded in a browser.
- Do not add a node/npm build step. Frontend is plain files referenced by `frontendDist`.
- Do not download the BetterTouchTool binary more than once; recon agent owns `docs/work/recon.md`.

## Definition of done (MVP)
1. `cargo check` exits 0 on Windows (stub path) and all mac-only code is behind `cfg`.
2. `cargo test` green, with unit tests covering: 3-finger tap fires, 4-finger does not, movement does not,
   long hold does not, cooldown blocks double fire, 2-finger does not.
3. UI: fixed 720×540 window, no resize handles, renders correctly in a browser, toggle + action row + permission state.
4. Config round-trips (save → reload → same values), invalid config falls back to defaults.
5. `docs/work/` holds charter, spec, plan, recon, impl-notes, test-cases, verify-report.
6. README with macOS build/run/permission steps.

## Shared resources
Only one agent claims the build slot. `docs/work/.build-lock` holds owner + purpose + timestamp while a
`cargo` command runs; anyone else defers and says so. Stale after 10 minutes.

## Gates
- Lead writes charter + architecture (done).
- BA → spec + plan. Dev → code. QA → independent verification. Lead → final review + report.
- No agent edits another role's files.
