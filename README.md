# Trio

A tiny macOS utility that does one thing: **tap the trackpad with three fingers → middle click**.

Built with Rust + Tauri 2 and a plain HTML/CSS/JS frontend (no bundler, no node build step).
Fixed 720×540 window, one card, one toggle.

## What it does

- Listens to raw multitouch contact frames through Apple's private
  `MultitouchSupport` framework (loaded at runtime, no link-time dependency).
- Detects a three-finger **tap** in a pure, unit-tested state machine
  (`src-tauri/src/gesture.rs`): three fingers down, short duration, almost no
  travel, then lift.
- Posts a real middle click (`CGEventOtherMouseDown`/`Up`, button 2) at the
  current cursor location through CoreGraphics.
- Stores settings in `~/Library/Application Support/app.trio.desktop/config.json`
  (atomic write: `config.json.tmp` → rename).

Everything is deliberately scoped: no shortcuts, no per-app bindings, no window
snapping. One gesture, done well.

## Requirements

- macOS 13 or newer (Apple Silicon or Intel).
- Xcode Command Line Tools (`xcode-select --install`).
- Rust stable (`rustup` recommended).
- Tauri CLI v2 (one of):
  - `cargo install tauri-cli --version "^2" --locked`, or
  - `npm install --save-dev @tauri-apps/cli` and use `npx tauri`.

## Run in development (macOS)

```bash
git clone <this repo> trio && cd trio

# Option A: cargo-installed CLI
cd src-tauri
cargo tauri dev

# Option B: node-installed CLI (still no frontend build step)
npx tauri dev
```

`frontendDist` points at `../src`, so the window loads `src/index.html`,
`src/styles.css` and `src/main.js` directly.

Convenience script:

```bash
./scripts/dev-mac.sh
```

## Build a release app

```bash
cd src-tauri
cargo tauri build
```

Artifacts land in `src-tauri/target/release/bundle/` (`Trio.app` and a `.dmg`).
The build is **unsigned and not notarized**. On another Mac, Gatekeeper may
refuse the first launch; either right-click → Open, or clear the quarantine bit:

```bash
xattr -dr com.apple.quarantine /Applications/Trio.app
```

## Grant Accessibility permission

Trio can observe touches without permission, but macOS blocks synthetic clicks
unless the app is trusted for Accessibility.

1. Launch Trio.
2. Click **Grant access** in the banner (Trio opens the right settings pane), or
   go to **System Settings → Privacy & Security → Accessibility**.
3. Enable **Trio** in the list (add it with `+` if it is not there yet).
4. macOS may ask you to quit and reopen the app — do that if clicks still do
   not arrive. Trio polls the permission every 2 seconds and updates the window
   without a restart, but macOS caches trust per process.

## Disable conflicting macOS gestures

macOS ships three-finger gestures that fire at the same time as Trio's tap. In
**System Settings → Trackpad → More Gestures**, turn off (or switch to four
fingers) anything assigned to three fingers, for example:

- Look up & data detectors
- Three finger drag (Accessibility → Pointer Control → Trackpad Options)
- Swipe between pages / Mission Control / App Exposé

Without this, a tap may also trigger a system gesture even though Trio's middle
click still fires.

## Verify (works on any OS)

```powershell
# Windows PowerShell
powershell -ExecutionPolicy Bypass -File scripts/check.ps1
```

or individually:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
cargo fmt   --manifest-path src-tauri/Cargo.toml --check
```

The gesture state machine is pure Rust with no platform code, so its tests run
on Windows/Linux too. On non-macOS targets `src-tauri/src/platform/stub.rs`
implements the same interface as the macOS engine.

## Preview the UI without the app

Open `src/index.html` directly in a browser (`file:///.../src/index.html`).
When `window.__TAURI__` is absent the page uses a localStorage-backed mock of the
same commands, so design review needs no toolchain. Preview mode is announced on
the console, not as on-screen copy (the UI text is fixed by the product spec).

Mock modes via query string:

```
src/index.html?mock=ok        # active, no banner (default)
src/index.html?mock=off       # gesture disabled
src/index.html?mock=noperm    # permission banner
src/index.html?mock=error     # listener error banner
src/index.html?mock=both      # error wins over permission
src/index.html?mock=savefail  # saving rejects with "disk full (mock)"
```

`window.__trioMock.calls` logs every mocked invoke; `window.__trioMock.push({...})`
merges a partial status and re-renders (e.g. `push({permission: "granted"})`).

## Troubleshooting

- `TRIO_DEBUG=1 cargo tauri dev` logs every parsed contact frame
  (`contacts=n stride=88 positions=[...]`) to stderr. Use it to confirm the
  stride/offset against real hardware.
- `TRIO_CONTACT_STRIDE=80` (or `96`) overrides the contact record stride if a
  macOS build uses a different size than the default 88.
- `TRIO_MT_PATH=/nonexistent cargo tauri dev` forces the private-framework load
  to fail, proving the error banner instead of a crash.
- Nothing fires? Check the Accessibility toggle first, then the listener banner
  (it shows the exact missing framework/symbol), then whether macOS owns the
  three-finger gesture.

## Architecture

```
src-tauri/
  src/main.rs              entry point
  src/lib.rs               Tauri builder, managed state, status events
  src/config.rs            frozen v1 config: load / clamp / atomic save
  src/contact.rs           stride-independent contact position parsing + tests
  src/gesture.rs           pure three-finger tap state machine + tests
  src/commands.rs          the whole invoke surface
  src/platform/mod.rs      platform boundary + event types
  src/platform/macos.rs    MultitouchSupport + CoreGraphics + AX (all dlopen)
  src/platform/stub.rs     no-op implementation for non-macOS builds
src/                       index.html, styles.css, main.js (no bundler)
scripts/                   check.ps1, dev-mac.sh, gen-icons.ps1
```

### Why no platform crates

`Cargo.toml` intentionally depends only on `tauri`, `serde` and `serde_json`.
Every macOS framework symbol — including `CFRelease`, `CFRunLoopRunInMode` and
`AXIsProcessTrusted` — is resolved at runtime with `dlopen`/`dlsym`. A missing or
renamed private symbol becomes the readable error status
(`listener: "error"`, `listener_error: "..."`) in the UI instead of a crash. This also
keeps `cargo check` on Windows free of any macOS code.

## Known limitations

- **Private framework.** `MultitouchSupport.framework` is undocumented and can
  change between macOS releases. If Apple changes it, Trio shows a listener
  error instead of crashing, but the gesture stops working until updated.
- **Unsigned app.** No Developer ID signature / notarization in this MVP.
- **Accessibility is mandatory** for the synthetic click; without it the UI
  shows the permission banner and taps do nothing.
- **System gestures overlap.** Three-finger macOS gestures must be disabled (see
  above) or they fire alongside the middle click.
- **`launch_at_login` exists in the schema but is not implemented yet.**
- **Closing the window hides it** (the app keeps running); clicking the Dock icon
  shows it again. **Quit** in the app menu (⌘Q) or the footer button exits.
- **Restarting the listener re-registers the callback** on the same devices
  (the private API has no unregister). The 250 ms cooldown keeps this harmless
  in practice, but it is a known rough edge.
- Contact positions are read at byte offset 24 of each record using a runtime
  stride (`TRIO_CONTACT_STRIDE`, default 88; 80 is the published variant).
  Unreadable/garbage positions are ignored by the movement guard instead of
  rejecting taps. Use `TRIO_DEBUG=1` to confirm the stride on a real machine.
- macOS only. The non-macOS build exists so the repo checks and tests anywhere;
  it does not implement the gesture.
