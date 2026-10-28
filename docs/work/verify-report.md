# Verification report — Trio v0.1 (QA, independent)

Verified snapshot: **2026-10-28 16:45:34** (`app-inventory.md` §0 has the hashes).
The implementer was still saving files when this review began; the macOS file changed
three times (16:43:04 → 16:43:12 → 16:45:34). Everything below was re-run against the
final snapshot. Nothing in `src/` or `src-tauri/` was edited by QA; the only writes were
these three `docs/work/` files (plus a transient `docs/work/.build-lock`).

Bottom line: **20/20 Windows-verifiable criteria pass** (5 of them only partially, as
marked), the crate builds and tests clean on Windows, and **the macOS half is unproven** —
worse, it was never even compiled by this team until this review built an isolated
cross-target check. Two open defects could plausibly break the core promise on a Mac, and
no test here can rule them out.

---

## 1. Commands actually run (verbatim, with real exit codes)

| # | command | exit | output summary |
|---|---|---|---|
| 1 | `cargo check --manifest-path E:\3touch\src-tauri\Cargo.toml` | **0** | `Finished dev profile … in 0.27s`, no warnings |
| 2 | `cargo test --manifest-path E:\3touch\src-tauri\Cargo.toml` | **0** | `28 passed; 0 failed; 0 ignored` ×1 suite, 2 empty doctest/bin suites |
| 3 | `cargo check --manifest-path E:\3touch\src-tauri\Cargo.toml --target aarch64-apple-darwin` | **101** | `error: failed to run custom build command for objc2-exception-helper v0.1.1` → `ToolNotFound: failed to find tool "cc"` |
| 4 | isolated cross-check (see §2.3) `cargo check --target aarch64-apple-darwin` | **0** | `Finished dev profile … in 0.18s` |
| 5 | `node --check src\main.js` | **0** | — |
| 6 | `node -e "JSON.parse(…tauri.conf.json, …capabilities/default.json)"` | **0** | `both valid` |
| 7 | browser harness, 6 mock modes + click/push/savefail probes (see §2.4) | n/a | all assertions observed; see §3 |

`#3` is not a defect in Trio: the `tauri` dependency tree pulls `objc2-exception-helper`,
whose build script needs a C compiler for the *target* (`cc` for Apple). This box has none,
so **the plan's "second gate" (`cargo check --target aarch64-apple-darwin`) does not work
for this crate** — only for dependency-free crates, as recon's throwaway test showed.
Consequence: `src/platform/macos.rs` is never type-checked by `cargo check` on Windows
(it is `cfg`-excluded) and cannot be type-checked in-tree here. That is exactly how D0
survived four revisions.

## 2. How the gaps were closed

### 2.1 The blocker that no gate would have caught

At the 16:43:12 revision, `macos.rs` called `.map_err(missing_symbol)?` on the result of
`resolve!`, but that macro returns the raw function pointer, not a `Result`. Four call
sites, four `E0599: no method named map_err found for fn pointer`. On a Mac that is a hard
build failure; on Windows it is invisible. Reported as D0; fixed by the implementer at
16:45:34 with a `try_resolve!` macro returning `Result`.

### 2.2 Confirming the rest of the file is sound

With only that one construct patched in a **copy** (`C:\Users\namth\AppData\Local\Temp\opencode\qa-mac-check`),
the remaining 660 lines type-checked for `aarch64-apple-darwin` (exit 0, one expected
`dead_code` warning for the then-unused helper). So the defective file had exactly one
root cause, not a pile of them — which is why the fix converged in one edit.

### 2.3 The standing cross-target gate

A dependency-free crate that includes the **real** files by path:

```
Cargo.toml:  [package] name="maccheck" edition="2021"   (no dependencies)
src/lib.rs:  pub mod gesture;  pub mod platform;
src/gesture.rs, src/platform/{mod,macos,stub}.rs   ← copied from src-tauri/src
```

`cargo check --target aarch64-apple-darwin` → **exit 0**. This proves the macOS engine,
the gesture machine and the platform boundary are *syntactically and type-*valid for the
Apple target with the real signatures. It does **not** cover `lib.rs`/`commands.rs`
(they need the Tauri tree, which cannot be cross-checked here) — the mac-only block in
`lib.rs` (`handle_run_event`, `RunEvent::Reopen`) remains uncompiled. Cheap residual gap,
small surface.

### 2.4 Browser evidence

`node` static server on `127.0.0.1:8791` serving `E:\3touch\src` plus one QA harness page
that mounts the app in six same-origin iframes at exactly **720×540**, waits for the mock's
async boot, then measures `scrollWidth/Height`, every hit box, pill/banner/caption state,
the painted text nodes, and drives clicks and `__trioMock.push()`.

## 3. Results

| area | rows | pass | partial | fail | mac-only |
|---|---|---|---|---|---|
| AC-1…AC-20 (Windows) | 20 | 15 | 5 | 0 | 0 |
| AC-21…AC-29 (Mac) | 9 | — | — | — | 9 |
| Gesture machine (G1…G14) | 14 | 14 | 0 | 0 | 0 |
| Config (C1…C8) | 8 | 8 | 0 | 0 | 0 |
| Rust surface (R1…R8) | 8 | 8 | 0 | 0 | 0 |
| Frontend (F1…F9) | 9 | 9 | 0 | 0 | 0 |
| Window (W1…W4) | 4 | 4 | 0 | 0 | 0 |
| **total** | **72** | **58** | **5** | **0** | **9** |

(The 72 includes the five partials twice in the AC line only if you count them as passes;
the row total is 20 + 9 + 14 + 8 + 8 + 9 + 4 = 72, of which 58 are clean passes.)
Partials: AC-11 (real pointer drag), AC-14 (toast fade timing + switch revert),
AC-15 (real key events), AC-16 (media-feature swap), AC-17 (computed duration).

Everything green on Windows that matters: `cargo check` 0, `cargo test` 0 with 28 tests,
`node --check` 0, both JSON files valid, all 31 spec strings verbatim, all 38 colour
tokens equal to spec, no scrollbar in any of six mock modes, hit targets all ≥ 28 px,
`trio:status` shape and event name matching between Rust and JS, mock/command parity exact.

---

## 4. Defects

Severity: **blocker** = breaks the MVP promise on a Mac or stops the build; **major** =
breaks a documented contract or can silently defeat the core feature; **minor** = visible
divergence with no functional loss.

| id | sev | file:line | reproduction | expected vs actual | evidence |
|---|---|---|---|---|---|
| D0 | blocker **(fixed in this snapshot)** | `src-tauri/src/platform/macos.rs:208-235` (rev 16:43:12) | isolated aarch64 check of the real file | expected: `resolve!` returns `Result`; actual: macro returns `$ty`, so `.map_err(missing_symbol)?` on four fields → `E0599` ×4, exit 101 | the operator's own output: `no method named map_err found for fn pointer` at lines 219/226 and two more. Fixed at 16:45:34 by `try_resolve!` (`macos.rs:189-194`). Current file: exit 0 |
| D1 | major | `src-tauri/src/lib.rs:97-106` | read the order: `platform::start(...)` spawns a thread (`macos.rs:484-509`) that reports failure through the sink (`macos.rs:492-494`), then `start_engine` **unconditionally** writes `engine.listener_error = None` when the returned `Result` is `Ok` | expected: the thread's error survives; actual: a listener that dies *after* spawn (no trackpad, no run-loop mode) can have its error erased and re-emitted as `listener:"stopped"` | the window is a few µs wide but deterministic under a fast failure; `Ok(())` from `start()` only means "thread spawned", not "listening" |
| D2 | major | `src-tauri/src/platform/macos.rs:71-100` | compare the field list with the published `MultitouchSupport` `Finger` layout; then note `const _: () = assert!(size_of::<MtContact>() == 88)` is `#[cfg(test)]` | expected: a test that can fail; actual: it asserts the size of the team's own definition, so it passes on Windows no matter what Apple ships. The code has **13 f32 + 9 i32 = 88 bytes**; the widely published layout has 11 f32 + 9 i32 = **80 bytes** (it carries one velocity pair, not two) | if 80 is right, `state` (offset 12) and `normalized_x/y` (24/28) are still correct for `contact[0]`, but the 88-byte stride makes every later contact garbage → finger count wrong → no tap ever. Unprovable here; see risk (a) |
| D3 | minor | `src-tauri/src/lib.rs:177` and `:201` | `Select-String -Path lib.rs -Pattern invoke_handler` → 2 hits at 177, 201 | expected: one registration; actual: two identical `generate_handler!` blocks. Tauri overwrites the field (`tauri-2.12.0/src/app.rs:1723`), so it is dead code, not a crash | two lists today; the day they diverge, the first is silently ignored |
| D4 | minor | `src/styles.css:538-546` | measure `#duration` at 720×540 | expected 300 px per spec §2; actual **295 px** | `(656 − 2×20 − 24) / 2 = 296`, minus sub-pixel rounding = 295. The spec assumed 16 px card padding; the card uses 20 px |
| D5 | minor | `src/index.html:61-147`, `styles.css:285` | measure `.card` | expected two cards (153 + 16 + 116); actual one card **285 px** with two `hr.divider`s | total vertical budget identical (285), anatomy differs; screenshots/harness agree |
| D6 | minor | `src-tauri/src/gesture.rs:92` vs `config.rs:21-22` | read both clamp tables | expected cooldown 100–1000 everywhere (spec §4.4); actual `Params::sanitized` uses `0..2000` | unreachable through `Config` (it clamps first), but two sources of truth exist |
| D7 | minor | `src-tauri/src/commands.rs:148-154` | read `request_permission` | expected: AX prompt variant **then** open the pane (spec §4.5); actual: opens the pane only, with a comment saying no C-API prompt is called | user gets one prompt instead of two; the pane still opens |
| D8 | minor | `src/main.js:131` | read | expected: select never disabled (spec §4.3); actual `disabled = !statusReady` | ≤ 1 frame in Tauri, invisible in practice |
| D9 | minor | `tauri.conf.json` / plan step 6 | `cargo check --target aarch64-apple-darwin` | expected: a second gate; actual: exit 101 from `objc2-exception-helper`'s `cc` lookup | gate only exists for dependency-free crates; documented in §1 #3 |
| D10 | minor | `src/styles.css:92-99` | measure `.spacer` | expected 123 / 59 px (spec §2); actual **75 / 23 px** | `gap: 16` applies between every flex child, so the banner's gap is double-counted when absent. No overflow (23 ≥ min 16) |

No defect was reported that was not reproduced from a command or a quoted line. The four
largest risks are *not* in this table because they cannot be settled without a Mac — they
are in §5.

---

## 5. Cannot verify without a Mac

### 5.1 Criteria

AC-21 (window), AC-22 (permission live update), **AC-23 (tap → middle click)**, **AC-24
(negatives)**, **AC-25 (timing)**, **AC-26 (toggle persistence)**, **AC-27 (hot params)**,
AC-28 (dlopen failure banner), AC-29 (idle CPU). AC-23…AC-27 are the entire product promise.

### 5.2 The four load-bearing unknowns, with what is evidence and what is assumption

**(a) The 88-byte contact struct and the callback signature.**
Evidence: the file's own layout sums to 88 and its `#[cfg(test)]` assertion passes — but
that assertion is a tautology about the team's own `struct` (D2). The field list carries
`vel_x/vel_y` *and* `abs_vx/abs_vy` plus `abs_x/abs_y`, i.e. six float "vector" slots where
the published layout has four (`size` sits at offset 32 in the published struct, at 40
here). Assumption: that Apple's stride is 88. If it is 80, `from_raw_parts(contacts, count)`
misparses every contact after the first and the feature never fires. The callback's first
parameter is also typed `*mut c_void` where the documented signature is `int device` — benign
only because the parameter is ignored (`_device`), and a latent trap for whoever uses it.
*Nothing on this box can settle the layout.*

**(b) Whether the run-loop pumping lets callbacks fire.**
Evidence: registration, `MTDeviceStart` and the pump all happen on one spawned thread
(`macos.rs:529-568`); the pump is `CFRunLoopRunInMode(mode, 1.0, 1)`. Assumption: that
MultitouchSupport delivers contact frames on the calling thread's run loop rather than the
main run loop. If it delivers on the main run loop, the app is silent and every unit test
still passes. Secondary suspicion: `returnAfterSourceHandled = 1` makes the call return as
soon as one source is handled, so a busy source can turn this into a hot spin — a plausible
cause of failing AC-29 (< 1 % CPU).

**(c) Whether `CGEventPost` with button 2 at the HID tap is a real middle click.**
Evidence: constants match CoreGraphics (`kCGEventOtherMouseDown/Up = 25/26`,
`kCGMouseButtonCenter = 2`), events are posted at the current cursor location with a 10 ms
gap (`macos.rs:385-426`), and recon found the same three primitives in BetterTouchTool's
binary. Assumption: that apps and the window server treat it as a middle click. Plausible,
untested. Note the click also needs AX trust — with it missing the banner explains why, but
the click path fails silently otherwise.

**(d) Whether a 3-finger tap reaches the app when macOS's own gesture is on.**
Evidence: the spec documents the conflict (§1) and the README tells users to disable
"Look up & data detectors" / three-finger drag (README L82-95). Assumption: that with those
off, the raw contact stream still arrives. If the OS consumes the tap earlier, the state
machine sees nothing and there is no error to display — the worst failure mode (silent).

Two smaller open risks: `MTDeviceStart(device, 0)`'s second argument is unvalidated, and the
device loop registers on **every** device returned by `MTDeviceCreateList` (no trackpad
filter) and ignores `MTDeviceStart`'s return, counting a failed start as "registered".

### 5.3 Cheapest manual check for a Mac owner (≈ 10 minutes)

1. `cd src-tauri && cargo tauri dev` — this alone settles D0-class problems and AC-21.
2. Open `https://mdn.github.io/dom-examples/auxclick/` (or any page logging `auxclick`) in
   Safari; note the button number.
3. `TRIO_DEBUG=1 TRIO_MT_PATH=/nonexistent cargo tauri dev` → expect the danger banner with
   `MultitouchSupport could not be loaded.` (AC-28, and it exercises D1's race).
4. `TRIO_DEBUG=1 cargo tauri dev`, then tap with 3, 4, 2 fingers and a 3-finger swipe while
   watching stderr: `[trio] frame contacts=N states=[…]` proves (b) and (d) immediately —
   if no frame lines appear, the run loop or the OS is swallowing the gesture and nothing
   else matters.
5. If frames appear but the tap never fires, print `size_of::<MtContact>()` next to the
   observed `count` and compare `states` — that is the 2-minute test for (a).

---

## 6. Verdict

Windows-verifiable surface: **sound**. Build, tests, config semantics, copy, tokens,
layout, hit targets, contract shape and mock parity all hold, and the earlier macOS
compile blocker is gone in the final snapshot.

Mac surface: **unproven, with two open majors**. Do not describe the MVP as done until a
Mac owner runs §5.3 step 4 — a single stderr line decides whether the feature can work at
all. The honest summary is: the team proved the detector and the UI, and has not yet proved
the two drivers between them (the contact feed and the posted click).
