# Test cases — Trio v0.1

Owner: QA. Written against the 16:45:34 snapshot (`app-inventory.md` §0).
`verify on` is `Windows` only when the check can be *mechanically* settled on this box
(cargo, file inspection, headless Chromium at 720×540). Everything needing a trackpad,
the Accessibility database or a real window server is `Mac-only`.

Result vocabulary: **PASS** (executed, observed), **PARTIAL** (executed, only part of the
criterion is observable here), **FAIL** (executed, wrong), **MAC-ONLY** (not executable),
**NOT RUN** (nobody executed it yet).

| id | area | precondition | steps | expected (exact) | pri | type | verify on | result | evidence |
|---|---|---|---|---|---|---|---|---|---|
| AC-1 | build | Windows, stable toolchain | `cargo check --manifest-path src-tauri/Cargo.toml` | exit 0 | P0 | build | Windows | PASS | exit 0, `Finished dev profile` |
| AC-2 | gesture | unit tests compile | `cargo test --manifest-path src-tauri/Cargo.toml` | exit 0, all green, includes the 8 named rules | P0 | unit | Windows | PASS | exit 0, `28 passed; 0 failed` |
| AC-3 | window | — | parse `tauri.conf.json` | 720×540, resizable false, maximizable false, center true, title `Trio`, withGlobalTauri true, frontendDist `../src` | P0 | config | Windows | PASS | all seven values asserted |
| AC-4 | config | — | run the 10 `config.rs` tests | round-trip equal, missing→default, corrupt→default, clamps, unknown fields ignored, no `.tmp` left | P0 | unit | Windows | PASS | 10/10 green |
| AC-5 | gesture | — | grep `gesture.rs` for `cfg(target_os`, `std::thread`, `crate::platform` | no hits | P0 | static | Windows | PASS | 0 hits |
| AC-6 | mac engine | — | grep for `#[link(name = "MultitouchSupport"`; check `platform/mod.rs` cfg gates | no link-time dep, `dlopen`/`dlsym` present, macos under `cfg(macos)`, stub otherwise | P0 | static | Windows | PASS | only `#[link(name = "System")]`; 10 dlopen/dlsym hits; both `cfg` branches |
| AC-7 | layout | browser | load `?mock=ok,off,noperm,error,both` at 720×540 | no scrollbar, nothing clipped | P0 | visual | Windows | PASS | all modes `scroll=720×540 = client`; card 285 inside app 496 |
| AC-8 | copy | browser, all modes | diff every painted text node against spec §5 | every §5 string present verbatim, no other visible text | P0 | visual | Windows | PASS | 31/31 present; visible set == spec set |
| AC-9 | gesture row | browser | inspect `#action` | `Middle click` selected, other 3 `disabled` | P1 | visual | Windows | PASS | 1 selected + 3 `disabled` counted in DOM |
| AC-10 | toggle | `?mock=ok` | click `#toggle` twice | `aria-checked` flips, pill `Active`↔`Off`, caption follows, exactly one `set_enabled` per click | P0 | visual | Windows | PASS | click1 → `false`/`Off`/`✓ Saved`, calls `[…,set_enabled]`; click2 → `true`/`Active`, 2 total |
| AC-11 | sliders | `?mock=ok` | drag then release; then keyboard step | label follows continuously; one `set_gesture_params` per release/step | P1 | visual | Windows | PARTIAL | `change` → exactly one `set_gesture_params` (harness); continuous label needs a real pointer drag, not synthesised |
| AC-12 | banners | `?mock=noperm,error,both` | load each | permission banner + paused switch + `Needs access`; error/both show only danger banner + reason + `Unavailable` | P0 | visual | Windows | PASS | noperm: banner visible, `paused=true`, pill `Needs access`; error/both: danger only, text `MultitouchSupport could not be loaded.`, pill `Unavailable` |
| AC-13 | live status | `?mock=noperm` | `__trioMock.push({permission:"granted"})` | banner removed, pill `Active`, no reload | P0 | visual | Windows | PASS | pill `Active`, `permission-banner.hidden=true` |
| AC-14 | toast | `?mock=ok`, `?mock=savefail` | persist; then fail a persist | `✓ Saved` then gone ≈1.8 s; savefail shows `Couldn't save: disk full (mock)` and reverts the switch | P1 | visual | Windows | PARTIAL | `✓ Saved` and the exact savefail string observed; the 1.6 s fade-out and switch revert were not timed |
| AC-15 | keyboard | browser | Tab through; Space on switch; arrows on sliders | Tab order = §8, 2 px accent ring at each stop, Space toggles, arrow step persists | P1 | a11y | Windows | PARTIAL | DOM/focusable order == §8 and `:focus-visible` ring exists in CSS; real key events not sent |
| AC-16 | dark mode | DevTools emulation | switch `prefers-color-scheme: dark` | every token swaps to the §3.1 dark column | P1 | visual | Windows | PARTIAL | all 19 dark values present and equal to spec; the media swap itself not exercised |
| AC-17 | reduced motion | DevTools emulation | switch `prefers-reduced-motion: reduce` | computed `transition-duration` `0.01ms` on switch, banner, toast | P2 | visual | Windows | PARTIAL | the `*` rule with `0.01ms !important` exists; computed value not read back |
| AC-18 | no modals | — | grep `main.js`/`index.html` | no `alert(`, `confirm(`, `prompt(`, `<dialog` | P0 | static | Windows | PASS | 0 hits |
| AC-19 | hit targets | browser | measure every visible interactive box | all ≥ 28×28 | P1 | visual | Windows | PASS | 44×28, 220×32, 295×28 ×2, 102×28, 56×28, banner buttons 108×98×57 ×28 |
| AC-20 | docs | — | read `README.md` | build, first run, AX grant, three-finger conflict | P1 | docs | Windows | PASS | sections at L32, L54, L69, L82, L95, L114, L135 |
| AC-21 | window | real Mac, macOS 13+ | launch, try to resize, hover the green button | 720×540 content, edges inert, zoom disabled | P0 | manual | Mac-only | MAC-ONLY | config claims it; the window server is not here |
| AC-22 | permission | Mac without AX trust | first run; click `Open Settings`; grant; wait | banner shows, pane opens at Privacy ▸ Accessibility, banner clears < 3 s, no restart | P0 | manual | Mac-only | MAC-ONLY | 2 s poll exists (`lib.rs:117-129`); grant path unproven |
| AC-23 | core gesture | Mac, Safari | 3-finger tap a link; tap a tab | background tab opens; tab closes | P0 | manual | Mac-only | MAC-ONLY | needs the private stream + a real trackpad |
| AC-24 | negatives | Mac, `auxclick` test page | 4-finger tap, 2-finger tap, 3-finger swipe, pinch | zero middle clicks | P0 | manual | Mac-only | MAC-ONLY | state machine says so; the OS feed does not exist here |
| AC-25 | timing | Mac | hold 3 fingers > 400 ms then lift; two taps < 250 ms apart | no click; exactly one click | P0 | manual | Mac-only | MAC-ONLY | covered in unit tests only |
| AC-26 | toggle | Mac | switch off, tap, on, tap, quit, relaunch | no clicks off; clicks on; no restart; state survives | P0 | manual | Mac-only | MAC-ONLY | persistence covered by `round_trips_through_disk` |
| AC-27 | hot params | Mac | set Tap window 150, tap ~300 ms | no click, no restart | P1 | manual | Mac-only | MAC-ONLY | `set_params` hot-swap exists |
| AC-28 | dlopen failure | Mac, `TRIO_MT_PATH=/nonexistent` | launch | no crash; danger banner `MultitouchSupport could not be loaded.` | P1 | manual | Mac-only | MAC-ONLY | string is returned by `resolve_mt`; the race in D1 may swallow it |
| AC-29 | idle CPU | Mac | 60 s in Activity Monitor | < 1 % CPU | P2 | manual | Mac-only | MAC-ONLY | see risk (b): the run-loop pump may busy-spin |
| G1 | 3-finger tap fires | default params | `push(0,[3]) push(60,[3]) push(120,[])` | `None,None,MiddleClick` | P0 | unit | Windows | PASS | `three_finger_tap_fires` |
| G2 | 4 fingers never fire | default | 4-finger candidate then lift | all `None` | P0 | unit | Windows | PASS | `four_finger_tap_does_not_fire` |
| G3 | 2 fingers never start | default | 2-finger frames then lift | all `None` | P0 | unit | Windows | PASS | `two_finger_tap_does_not_fire` |
| G4 | 5 fingers never fire | finger_count 3 | 5-finger candidate then lift | all `None` | P1 | unit | Windows | PASS | `five_fingers_do_not_fire_when_finger_count_is_three` |
| G5 | movement > tolerance | default 0.08 | centroid jumps 0.3 then lift | `None` | P0 | unit | Windows | PASS | `movement_beyond_tolerance_does_not_fire` |
| G6 | move and return | default | out 0.4, back to origin, lift | `None` (max travel is kept) | P1 | unit | Windows | PASS | `movement_back_to_origin_does_not_fire` |
| G7 | hold > max_duration | default 400 | 650 ms candidate, lift | `None` | P0 | unit | Windows | PASS | `long_hold_does_not_fire` |
| G8 | cooldown blocks double fire | default 250 | tap, tap again within 250 ms, then after | `MiddleClick, None, MiddleClick` | P0 | unit | Windows | PASS | `cooldown_blocks_double_fire` |
| G9 | cooldown boundary | cooldown 0 | back-to-back taps | both fire | P2 | unit | Windows | PASS | `cooldown_zero_allows_back_to_back_taps` |
| G10 | resting fingers still fire | default | 301 stationary 3-finger frames then lift | `MiddleClick` at lift | P0 | unit | Windows | PASS | `resting_finger_frames_still_fire` |
| G11 | lifting order irrelevant | default | reorder contacts mid-candidate, lift partially | fires on the first <3 frame | P1 | unit | Windows | PASS | `lifting_order_does_not_matter` |
| G12 | peak 4 then 3 | default | 3→4→3→lift | `None` | P1 | unit | Windows | PASS | `peak_of_four_then_three_does_not_fire` |
| G13 | `reset()` clears state | default | start candidate, `reset()`, feed lift | no click; cooldown also cleared | P1 | unit | Windows | PASS | `reset_clears_candidate_and_cooldown` |
| G14 | param change keeps cooldown | default | fire, change finger_count, tap | new params apply, cooldown still enforced | P2 | unit | Windows | PASS | `param_change_drops_candidate_but_keeps_cooldown` |
| C1 | defaults | — | `Config::default()` | 1 / true / 3 / middle_click / 400 / 0.08 / 250 / false | P0 | unit | Windows | PASS | `defaults_match_frozen_schema` |
| C2 | save→load | temp dir | save then load | equal structs, zero warnings, no `.tmp` | P0 | unit | Windows | PASS | `round_trips_through_disk` |
| C3 | missing file | temp dir | load a non-existent path | defaults, zero warnings | P0 | unit | Windows | PASS | `missing_file_yields_defaults_without_warnings` |
| C4 | corrupt JSON | temp dir | load `{ this is not json` | defaults + exactly one `could not parse` warning, no panic | P0 | unit | Windows | PASS | `corrupt_file_yields_defaults_with_one_warning` |
| C5 | clamping | — | parse out-of-range document | version→1, fingers→3, action→middle_click, duration→100, movement→0.30, cooldown→2000, 6 warnings | P0 | unit | Windows | PASS | `out_of_range_values_are_clamped` |
| C6 | unknown fields | — | parse with extra keys | ignored | P1 | unit | Windows | PASS | `unknown_fields_are_ignored` |
| C7 | partial document | — | parse `{"enabled": false}` | other fields default | P1 | unit | Windows | PASS | `partial_document_uses_field_defaults` |
| C8 | negative numbers | — | parse negative duration/cooldown | clamp to 100 / 0 | P2 | unit | Windows | PASS | `negative_durations_clamp_instead_of_failing_the_document` |
| R1 | no panics in mac engine | — | grep `macos.rs` for `unwrap()/expect(/panic!/todo!/unreachable!` | none | P0 | static | Windows | PASS | 0 hits |
| R2 | every load failure returns `Err` | — | read `open_framework`/`lookup`/`resolve_*` | all failure paths `Err`, no abort | P0 | static | Windows | PASS | `macos.rs:154-252`; `OnceLock<Result<…>>` caches the error |
| R3 | mac code fully gated | — | read `platform/mod.rs` | `macos` only under `cfg(macos)`, `stub` otherwise | P0 | static | Windows | PASS | lines 37-45 |
| R4 | dependency budget | — | read `Cargo.toml` | only tauri, serde, serde_json (+ tauri-build) | P0 | static | Windows | PASS | 3 runtime deps, 1 build dep |
| R5 | no link-time MT dependency | — | grep `#[link` | only `name = "System"` | P0 | static | Windows | PASS | `macos.rs:25` |
| R6 | mac-only code type-checks | — | isolated aarch64-apple-darwin check of `gesture.rs` + real `platform/{mod,macos,stub}.rs` | exit 0 | P0 | build | Windows | PASS | exit 0 (see `verify-report.md` §2.3); full-crate cross-check fails for environmental reasons |
| R7 | mock covers every invoked command | — | extract `invoke("…")` and `case "…"` | identical sets, all registered in Rust | P1 | static | Windows | PASS | 9/9/12 checked, zero orphans |
| R8 | status shape == spec | — | read `Status` struct + event name | `permission` string union, `listener` string union, `listener_error`, `trio:status` | P0 | static | Windows | PASS | `lib.rs:30-38, 74`; frontend `main.js:89-91, 341` |
| F1 | syntax | — | `node --check src/main.js` | exit 0 | P0 | build | Windows | PASS | exit 0 |
| F2 | copy parity | — | machine diff of 31 §5 strings | all present verbatim | P0 | static | Windows | PASS | 0 missing |
| F3 | no network / no external asset | — | grep for `http(s)://`, `@import`, `fetch(`, external `src`/`href` | only `styles.css`, `main.js` | P0 | static | Windows | PASS | 2 local refs |
| F4 | mock modes | browser | `?mock=ok,off,noperm,error,both,savefail` | each renders its documented state | P0 | visual | Windows | PASS | see AC-7/12/14 rows |
| F5 | tab order | — | parse DOM order of focusables | retry > open-settings > check-again > toggle > action > duration > movement > reset > quit | P1 | static | Windows | PASS | matches §8 once banners are hidden |
| F6 | focus ring | — | read CSS | `:focus-visible { outline: 2px solid var(--accent); offset 2px }` | P1 | static | Windows | PASS | `styles.css:747-754`; slider uses a thumb ring |
| F7 | 28 px minimum | browser | measure visible controls | none < 28 in either axis | P1 | visual | Windows | PASS | measured list in `app-inventory.md` §2 |
| F8 | no modal APIs | — | grep | none | P0 | static | Windows | PASS | 0 hits |
| F9 | one banner at a time | browser, `both` | load | danger banner only | P0 | visual | Windows | PASS | permission banner `hidden=true` while error shows |
| W1 | window config | — | parse config | 720×540, `resizable:false`, `maximizable:false`, `center:true`, title `Trio` | P0 | static | Windows | PASS | asserted |
| W2 | capabilities | — | parse `capabilities/default.json` | valid JSON, `windows:["main"]`, permissions exactly `["core:default"]` | P0 | static | Windows | PASS | asserted |
| W3 | no scroll at 720×540 | browser | load all modes | `scrollWidth/Height == client` | P0 | visual | Windows | PASS | 6/6 modes |
| W4 | frontendDist | — | parse config | `../src` | P0 | static | Windows | PASS | asserted |

## Totals

| bucket | count |
|---|---|
| rows | 72 |
| PASS | 58 |
| PARTIAL | 5 (AC-11, AC-14, AC-15, AC-16, AC-17) |
| FAIL | 0 |
| MAC-ONLY | 9 |

Arithmetic: AC-1…AC-20 = 15 PASS + 5 PARTIAL; G 14, C 8, R 8, F 9, W 4 = 43 PASS;
AC-21…AC-29 = 9 MAC-ONLY. 58 + 5 + 9 = 72.

## Rules with no test (gesture state machine audit)

None. Every bullet in `charter.md` §Gesture detection rules has a named test:

| charter rule | test |
|---|---|
| starts on `len() >= 3` | every candidate test; `empty_frames_are_harmless` for the negative |
| `peak_fingers = max(len)` | `peak_of_four_then_three_does_not_fire` |
| ends when it drops below 3 | `intermediate_lift_breaks_the_candidate`, `lifting_order_does_not_matter` |
| `peak_fingers == finger_count` | G2, G3, G4 |
| duration `<= max_duration_ms` | G7 |
| centroid travel `<= max_movement` | G5, G6 |
| cooldown | G8, G9 |
| lifting order irrelevant | G11 |
| resting fingers still fire | G10 |
| exception → `None`, state reset, no leaks | `empty_frames_are_harmless`, `out_of_order_timestamps_saturate_instead_of_wrapping` |
| `reset()` | G13 |
