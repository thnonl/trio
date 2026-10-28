# Impl notes — Coder (Trio v0.1 MVP)

Status: MVP implemented; Windows gates clean. macOS runtime unverified (no Mac).
UI follows `spec.md` (tokens/copy/ranges/status contract); `charter.md` and Lead
rulings win on conflicts. Fix round D1–D4 applied on top of QA's 72/72 pass.

## Fix round (QA defects)

### D1 — listener error lost — FIXED

- `src-tauri/src/lib.rs:91` `start_engine` no longer clears anything on the
  optimistic path: it marks `ready = false`, starts, then waits for confirmation.
- `src-tauri/src/lib.rs:134` `finish_listener_attempt(ready, error)` is the only
  place that writes the outcome; it runs after a confirmed `Ok` or a failure.
- `src-tauri/src/lib.rs:58` `status_now` reports `running` only when
  `ready && platform::is_running()`; an error always wins.
- `src-tauri/src/platform/mod.rs:46` new `ListenerHandle` (mpsc readiness +
  `Arc<AtomicBool>` alive + stored `last_error`); `:83` `wait_ready(150ms)`:
  message → Ok/Err, timeout → Ok only if alive, otherwise Err(reason).
- `src-tauri/src/platform/macos.rs:458` `start` returns the handle; `:539`
  `run_device_loop` sends `Ok(())` only after the devices are registered, and the
  thread stores/sends the `Err` reason before emitting `Event::Error`.
- Why safe: a thread that dies right after spawn can no longer look healthy; the
  error survives and `trio:status` is emitted with `listener:"error"`. No sleeps
  used as synchronisation (one 150 ms `recv_timeout`, as specified).
- Mac must confirm: device registration normally completes well inside 150 ms;
  if not, the timeout branch reports provisional running while the thread is
  alive (allowed by the fix spec).

### D2 — contact stride assumption — FIXED

- `src-tauri/src/contact.rs` new pure module (no cfg): `POSITION_OFFSET = 24`,
  `ContactPos { x, y }`, `plausible()` (−0.25…1.25, finite), `parse(base, count,
  stride)`; `:48`.
- `src-tauri/src/platform/macos.rs:60` `contact_stride()` from
  `TRIO_CONTACT_STRIDE` (`80|88|96`, default 88, comment notes 80 is the
  published variant); `:89` `MtContact` is now opaque — no field layout, no
  size assumption; `:641` `on_contacts` is a thin unsafe wrapper around
  `contact::parse`.
- `src-tauri/src/gesture.rs:222` centroid accumulates only from `is_plausible()`
  (`:39`) contacts; the candidate starts even with zero plausible positions and
  keeps its previous centroid, so an unknown stride degrades to "position
  unknown", never "tap rejected".
- Tests: 6 in `contact.rs` (80/88/96 give identical positions from offset 24,
  offset-only read, garbage bytes implausible, boundary/NaN/inf, zero/negative
  count, null base/impossible stride) + 3 in `gesture.rs` (unreadable positions
  still fire, junk frames are skipped for movement, first readable origin wins).
- Why safe: no code depends on the record size; only a fixed offset inside the
  shared 6×i32 header is read.
- Mac must confirm: the position really is at offset 24 on the target macOS and
  whether counting every record (no `state` filter any more) causes lingering
  contacts to inflate the peak. `TRIO_DEBUG=1` prints count/stride/positions.

### D3 — busy spin — FIXED

- `src-tauri/src/platform/macos.rs:585` pumps with
  `CFRunLoopRunInMode(mode, 1.0e10, 0)` (long timeout, no
  return-after-source) inside the generation-checked loop.
- Fallback when the run-loop symbol or default mode is unreachable: 1 s sleep
  loop with the same generation check (`macos.rs:589`+).
- Why safe: a contact callback no longer makes the pump return, so the thread
  blocks instead of spinning; shutdown still invalidates the generation and
  clears `CTX` (callbacks re-read it), so `retry_listener` works. The old thread
  may stay parked (private API has no unregister); it no longer touches state.
- Mac must confirm: AC-29 CPU over 60 s, and that callbacks are delivered while
  the run loop is parked (they should be, since MT registers run-loop sources).

### D4 — self-referential size assert — FIXED

- Removed `const _: () = assert!(size_of::<MtContact>() == 88)`.
- `macos.rs:89` now documents the 80/88/96 variants and the
  `TRIO_CONTACT_STRIDE` override; no `debug_assert!` because there is no
  meaningful invariant left to compare.

## Files

| Path | Purpose |
|---|---|
| `src-tauri/Cargo.toml` | Package `trio`, lib `trio_lib`; deps only `tauri`, `serde`, `serde_json` (+`tauri-build`). |
| `src-tauri/build.rs` | `tauri_build::build()`. |
| `src-tauri/tauri.conf.json` | 720×540 fixed window, `withGlobalTauri`, `frontendDist: ../src`, `app.trio.desktop`. |
| `src-tauri/capabilities/default.json` | Only `core:default` for `main`. |
| `src-tauri/src/main.rs` | Entry point. |
| `src-tauri/src/lib.rs` | Builder, state, readiness-gated engine start, `trio:status`, hide-on-close, Dock reopen. |
| `src-tauri/src/config.rs` | Frozen v1 schema, clamp (150–800 ms / 0.02–0.20 / cooldown 100–1000), atomic save. |
| `src-tauri/src/contact.rs` | Stride-independent contact position parsing + tests. |
| `src-tauri/src/gesture.rs` | Pure tap machine; plausible-only centroids. |
| `src-tauri/src/commands.rs` | Invoke surface (spec + lead aliases). |
| `src-tauri/src/platform/mod.rs` | Seam, `Event`, `EventSink`, `Permission`, `ListenerHandle`. |
| `src-tauri/src/platform/macos.rs` | MT + CG + AX via dlopen; runtime stride; blocking run loop. |
| `src-tauri/src/platform/stub.rs` | Same API on non-macOS (`permission = unsupported`). |
| `src-tauri/icons/*` | Generated by `scripts/gen-icons.ps1`. |
| `src/{index.html,styles.css,main.js}` | One-card UI, spec tokens/copy, mock modes. |
| `scripts/{check.ps1,dev-mac.sh,gen-icons.ps1}` | Gates, dev wrapper, icon generation. |
| `README.md` | Build/run/permission/gesture-conflict/troubleshooting. |

## Preserved decisions

- Tokens/type/motion from spec §3; copy from §5; slider ranges 150–800 / 2–20%;
  status record `{enabled, permission, listener, listener_error}` on `trio:status`.
- One card (charter/brief) with a Fine-tuning block, not the spec's two cards.
- dlopen-only platform layer; no platform crates; listener restart is
  generation-guarded; `Ctx` leaked so late callbacks never see freed memory.
- Close hides + Dock reopen shows; Quit/⌘Q exits via the default macOS menu.
- `TRIO_MT_PATH` and `TRIO_DEBUG` retained; `TRIO_CONTACT_STRIDE` added.

## Gate results (Windows 11, cargo/rustc 1.98.1, node 24.22.1)

| Command | Exit | Note |
|---|---|---|
| `cargo check --manifest-path src-tauri/Cargo.toml` | 0 | 0 warnings (stub path) |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | **42 passed**, 0 failed (28 + 14 new) |
| `cargo fmt --check` | 0 | clean |
| `node --check src/main.js` | 0 | unchanged this round |
| `rustc --target aarch64-apple-darwin --emit=metadata` on `gesture.rs` + `contact.rs` + `platform/mod.rs` (→ real `macos.rs`, `stub` cfg'd out) | 0 | isolated Apple-target type-check, temp harness in `%TEMP%\opencode\trio-mac-check` |
| `cargo check --target aarch64-apple-darwin` (full) | not run this round | still blocked in dependency `objc2-exception-helper`: host has no `cc`/clang; per instruction no toolchain was installed |

## What a Mac still must confirm

1. `listener` flips to `running` only with a real trackpad; unplugging/no-device
   path shows the exact error and survives.
2. Contact position offset/stride on the target macOS; whether lingering
   contacts inflate the peak now that the `state` filter is gone.
3. CPU < 1% idle with the blocking pump (AC-29), and callback delivery while the
   run loop is parked.
4. Middle-click delivery under Accessibility; 3-finger system gestures off.
5. macOS-only `lib.rs` glue (`RunEvent::Reopen`, `prevent_close`, menu ⌘Q) — not
   covered by the isolated harness.
