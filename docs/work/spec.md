# Spec — Trio v0.1 (MVP)

Owner of this file: BA. Source of truth above this: `charter.md` (wins on conflict).
Audience: Dev + QA agents. Every number below either comes from the charter or shows its arithmetic.

---

## 1. Product framing

- **For:** Mac trackpad users who need a middle click (open link in new tab, close tab, paste on Linux VMs, CAD/pan tools) and do not want a full gesture suite.
- **One job:** a quick 3-finger tap anywhere on the trackpad posts a middle click at the cursor.
- **Why not BetterTouchTool:** BTT configures hundreds of triggers per app. Its tap triggers are built on the same private MultitouchSupport stream, which is why we read that stream too (charter §macOS engine). We take that one trigger and add nothing else.
- **Out of scope for MVP:** other gestures or finger counts, other actions (they appear disabled in the UI only to show the app can grow), per-app rules, keyboard shortcuts, window snapping, a tray/menu-bar icon, launch at login UI (the `launch_at_login` field is stored but not shown), auto-update, telemetry, localisation.
- **Known OS conflict (README must say so):** macOS *Trackpad › Point & Click › Look up & data detectors → "Tap with three fingers"* uses the same gesture. Users must switch it off or set it to "Force click with one finger". Trio does not change system settings.

---

## 2. Information architecture (single window, 720 × 540, fixed)

The window uses standard macOS decorations, title `Trio`. 720 × 540 is the **inner (web content) size**; the title bar sits outside it.

Outer padding: **top 24, bottom 20, left 32, right 32** → content box **656 × 496** (720 − 32 − 32 = 656; 540 − 24 − 20 = 496).

Layout is a vertical flex column. The footer is pinned to the bottom. The other regions are top-aligned under the header.

| # | Region | Height px | Contents |
|---|---|---|---|
| 1 | Header | 44 | app glyph 32×32, title + subtitle, status pill (right) |
| 2 | gap | 16 | |
| 3 | Banner slot (conditional) | 52 | at most ONE banner: listener error beats permission |
| 4 | gap (only with a banner) | 12 | |
| 5 | Main card | 153 | 20 pad + toggle row 40 + 16 + divider 1 + 16 + gesture row 40 + 20 pad |
| 6 | gap | 16 | |
| 7 | Fine-tuning card | 116 | 16 pad + title row 20 + 12 + slider row 52 + 16 pad |
| 8 | flex spacer | ≥ 16 | absorbs the slack |
| 9 | Footer | 28 | version (left), toast slot (centre), Quit (right) |

**Sum with a banner:** 44 + 16 + 52 + 12 + 153 + 16 + 116 + 16 + 28 = **453 ≤ 496**. Slack is 43, so the spacer is 59.
**Sum without a banner:** 453 − 52 − 12 = **389**. Slack is 107, so the spacer is 123.
No state can scroll. `html, body { overflow: hidden }` is a guard, not the mechanism.

Horizontal: cards are 656 wide. In the fine-tuning card the two sliders sit **side by side**: (656 − 2×16 pad − 24 gap) / 2 = **300 px each**.

Row internals:
- Toggle row 40: label (15/20) + caption (12/16) = 36 ≤ 40; switch on the right.
- Gesture row 40: chip 32 high · arrow · select 32 high (220 wide); right-aligned caption.
- Slider row 52: label/value line 20 + gap 4 + range input 28 (track 4, thumb 16) = 52.
- Banner 52: padding 9 + title 18 + body 16 + padding 9 = 52; buttons are 28 high, vertically centred.

---

## 3. Design system

### 3.1 Colour tokens (CSS custom properties; dark mode via `@media (prefers-color-scheme: dark)`)

| Token | Role | Light | Dark |
|---|---|---|---|
| `--bg` | window background | `#F5F5F7` | `#1C1C1E` |
| `--surface` | cards | `#FFFFFF` | `#2C2C2E` |
| `--surface-raised` | select, chip (dark), switch knob base | `#FFFFFF` + shadow | `#3A3A3C` |
| `--chip-bg` | gesture chip fill | `#F5F5F7` | `#3A3A3C` |
| `--border` | card/control hairline (decorative) | `#D2D2D7` | `#48484A` |
| `--text` | primary text | `#1D1D1F` | `#F5F5F7` |
| `--text-muted` | captions, secondary | `#6E6E73` | `#98989D` |
| `--text-disabled` | disabled labels/options | `#8E8E93` | `#6E6E73` |
| `--accent` | non-text fills: switch on, slider fill, focus ring | `#0A84FF` | `#0A84FF` |
| `--accent-strong` | filled button background | `#0066CC` | `#0A6CDB` |
| `--accent-text` | text links / text buttons | `#0066CC` | `#409CFF` |
| `--accent-contrast` | text on `--accent-strong` | `#FFFFFF` | `#FFFFFF` |
| `--switch-off` | switch track, off | `#E5E5EA` | `#48484A` |
| `--switch-off-border` | switch track outline, off | `#8A8A8E` | `#8E8E93` |
| `--success` | "Active" dot/text, "Saved" toast | `#1E7B34` | `#30D158` |
| `--warning` | warning title/icon | `#8A3B00` | `#FFD60A` |
| `--warning-bg` | warning banner fill | `#FFF4E5` | `#3D3418` |
| `--danger` | danger title/icon, error toast | `#D70015` | `#FF6961` |
| `--danger-bg` | danger banner fill | `#FDECEC` | `#3D1F1E` |

Rule: in dark mode, muted text never sits on `--surface-raised` (#98989D on #3A3A3C = 3.95, which fails; see §8).
No gradients. No translucency/vibrancy in the MVP (the webview cannot rely on it).

### 3.2 Typography
Stack: `-apple-system, "SF Pro Text", "Helvetica Neue", sans-serif`. Numbers in values use `font-variant-numeric: tabular-nums`.

| Style | px | weight | line-height | letter-spacing | Used for |
|---|---|---|---|---|---|
| title | 20 | 600 | 24 | −0.2px | "Trio" |
| headline | 15 | 600 | 20 | −0.1px | toggle label, card title |
| body | 13 | 400 | 18 | 0 | select, banner title (600), buttons (500) |
| body-strong | 13 | 600 | 18 | 0 | slider values, banner title |
| caption | 12 | 400 | 16 | 0 | subtitles, captions, banner body, footer |
| micro | 11 | 600 | 14 | 0.2px | status pill |

### 3.3 Spacing, radii, elevation
- Spacing scale (px): `2, 4, 8, 12, 16, 20, 24, 32`. No other values except the 1px divider and the 9px banner padding (derived in §2).
- Radii: control 6 (buttons), 8 (select, chip, banner), 12 (cards), 999 (pill, switch).
- Card light: `background: var(--surface); border: 1px solid var(--border); box-shadow: 0 1px 2px rgba(0,0,0,0.06);`
- Card dark: same border token, **no shadow**.
- Raised control (select) light: `border: 1px solid var(--border); box-shadow: 0 1px 1px rgba(0,0,0,0.04);`. Dark: `background: var(--surface-raised); border: 1px solid var(--border)`.
- Switch knob: 18×18 white `#FFFFFF`, `box-shadow: 0 1px 2px rgba(0,0,0,0.25)` in both themes.
- Focus ring: `outline: 2px solid var(--accent); outline-offset: 2px;` on `:focus-visible` only.

### 3.4 Motion
| What | Duration | Easing |
|---|---|---|
| hover/active colour | 120ms | `ease-out` |
| switch knob + track | 160ms | `cubic-bezier(0.2, 0, 0, 1)` |
| banner enter | 200ms opacity 0→1 + translateY(−4px→0) | `cubic-bezier(0.2, 0, 0, 1)` |
| banner exit | 120ms opacity | `ease-in` |
| toast in / out | 160ms opacity | `ease-out` |
| toast dwell (success) | 1600ms | — |

Reduced motion: `@media (prefers-reduced-motion: reduce) { *, *::before, *::after { transition-duration: 0.01ms !important; animation-duration: 0.01ms !important; } }`. Nothing ever loops or pulses.

---

## 4. Components: anatomy and states

Global rule when **permission is missing**: every control stays operable and keeps persisting config. Nothing is greyed out because of permission. The banner and the "paused" switch state explain why taps do nothing.

### 4.1 Header (non-interactive)
Glyph: 32×32 rounded-8 square in `--accent` with three 6px white dots in a row (inline SVG). Title `Trio`, subtitle below. On the right, the **status pill**: height 22, padding 0 8, radius 999, 1px border `--border`, background `--surface`, a 6px dot plus micro text.

| Condition (from Rust status) | Pill text | Dot / text colour |
|---|---|---|
| no status received yet | `Starting…` | `--text-muted` |
| `enabled=false` | `Off` | `--text-muted` |
| `enabled` and `listener=error` | `Unavailable` | `--danger` |
| `enabled` and `permission≠granted` | `Needs access` | `--warning` (light: dot `#B25000`, text `--warning`) |
| `enabled`, `granted`, `running` | `Active` | `--success` |

The rows are evaluated top to bottom; the first match wins.

### 4.2 Enable switch (toggle row)
Anatomy: label + caption on the left; `<button role="switch" aria-checked>` on the right. The visual track is 38×22 with an 18px knob and 2px inset. The hit box is 44×28.

| State | Visual |
|---|---|
| off | track `--switch-off` + 1px `--switch-off-border`, knob left |
| on | track `--accent`, no border, knob right |
| paused (`enabled` but permission missing or listener error) | track `--accent` at `opacity: 0.45`, knob right, `aria-checked="true"` |
| hover | track brightness: off `#D1D1D6` (light) / `#545456` (dark); on: no change |
| focus-visible | focus ring around the 38×22 track |
| active (pressed) | knob widens to 22px toward the travel direction |
| disabled | only before the first status arrives: `opacity 0.4`, `aria-disabled="true"`, clicks ignored |
| permission missing | operable; turning it on shows "paused" |

Behaviour: the click flips the switch optimistically → `invoke('set_enabled', {enabled})`. Rust applies it to the engine immediately and persists it. On reject: revert the switch and show the error toast. The caption text follows the state (§5).

### 4.3 Gesture row
`[chip] → [select]`, then a right-aligned caption.
- **Chip** (non-interactive `<span>`): height 32, padding 0 12, radius 8, `--chip-bg`, three 5px `--text` dots + text `3 fingers` (body 500). It has no hover or focus states.
- **Arrow:** `→` U+2192, 16px, `--text-muted`, 8px margin each side, `aria-hidden="true"`.
- **Select:** native `<select id="action">`, 220×32, radius 8, raised-control recipe, padding 0 28 0 10, custom chevron via background SVG.
  - Options, in order: `Middle click` (value `middle_click`, selected, enabled), then disabled options `Left click — soon`, `Right click — soon`, `Mission Control — soon`. Disabled options use `disabled` (WebKit renders them greyed).
  - States: hover border `--text-muted`; focus-visible ring; active = native popup; disabled: never (the select is always enabled); permission missing: unchanged.
  - Changing it is a no-op because only one enabled option exists. The `change` handler still invokes `set_action` for forward-compat; Rust accepts only `middle_click`.

### 4.4 Fine-tuning sliders
Card title row: `Fine-tuning` (headline) on the left. On the right, the text button `Reset to defaults` (`--accent-text`, body 500). Its visual height is 20, with padding to give a 28px hit box and `margin: -4px 0` so the row stays 20.

Two `<input type="range">`, 300 wide, 28 high:

| Slider | Config key | min | max | step | default | Value label |
|---|---|---|---|---|---|---|
| Tap window | `gesture.max_duration_ms` | 150 | 800 | 10 | 400 | `{n} ms` |
| Movement tolerance | `gesture.max_movement` | 0.02 | 0.20 | 0.01 | 0.08 | `{round(v*100)}%` |

**These ranges are the clamp ranges in `config.rs` too (binding).** `cooldown_ms` has no UI. It is clamped to 100–1000 (default 250). `finger_count` is clamped to exactly 3 in v1, and `action` to `middle_click`.

| State | Visual |
|---|---|
| default | track 4px `--switch-off`; filled part `--accent`; thumb 16px white + knob shadow |
| hover | thumb shadow `0 1px 3px rgba(0,0,0,0.35)` |
| focus-visible | focus ring on the thumb (WebKit: `::-webkit-slider-thumb` outline) |
| active (dragging) | thumb scale 1.1 |
| disabled | only before the first status arrives: opacity 0.4 |
| permission missing | operable |

Behaviour: on `input`, update the value label only. On `change` (pointer release, or each keyboard step), `invoke('set_gesture_params', {max_duration_ms, max_movement})`. Rust persists the values and hot-swaps them into the engine. `Reset to defaults` sets 400 / 0.08 and persists once. Reset is also a text button with hover underline, a focus ring, and active opacity 0.7.

### 4.5 Permission banner (warning)
Shown when `permission ≠ "granted"` and there is no listener error, whether `enabled` is on or off. Banner: 52 high, radius 8, `--warning-bg`, 16px warning icon (triangle, `--warning`), title (`--warning`, body-strong), body (`--text`, caption, one line, ellipsis). Buttons on the right:
- Primary `Open Settings`: 28 high, padding 0 12, radius 6, `--accent-strong` bg, `--accent-contrast` text. Hover: brightness 1.08. Active: brightness 0.92. Focus ring.
- Secondary `Check again`: 28 high, transparent bg, 1px `--border`, `--text`. Hover: bg `rgba(0,0,0,0.04)` light / `rgba(255,255,255,0.06)` dark.

`Open Settings` → `invoke('request_permission')`. That calls the AX prompt variant, then opens `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`. `Check again` → `invoke('get_status')`. While permission is missing, Rust re-checks every 2000ms itself and pushes the change. No restart.

### 4.6 Listener-error banner (danger)
Shown when `listener = "error"`. It takes the banner slot ahead of the permission banner. Same anatomy with `--danger-bg` / `--danger` and a circle-exclamation icon. Body = `listener_error` verbatim, truncated with ellipsis; the full text goes in `title=`. One button: secondary `Retry` → `invoke('retry_listener')`.

### 4.7 Save feedback (footer toast)
Footer centre, `role="status" aria-live="polite"`, caption text, no box.
- Success: `✓ Saved` in `--success`. It shows after every successful persisting invoke, stays 1600ms, then fades. A new save restarts the timer.
- Failure: `Couldn't save: {reason}` in `--danger`. It stays until the next successful save.

Footer left: `Trio 0.1.0` (caption, `--text-muted`). Footer right: secondary button `Quit` (28 high) → `invoke('quit')`.

---

## 5. UI copy (verbatim; nothing else visible)

| Id | String |
|---|---|
| window.title | `Trio` |
| header.title | `Trio` |
| header.subtitle | `Three-finger tap → middle click` |
| pill.starting | `Starting…` |
| pill.off | `Off` |
| pill.active | `Active` |
| pill.needsAccess | `Needs access` |
| pill.unavailable | `Unavailable` |
| toggle.label | `Enable Trio` |
| toggle.caption.on | `Tap with three fingers anywhere on the trackpad.` |
| toggle.caption.off | `Off. Your trackpad works as usual.` |
| toggle.caption.pausedPermission | `Paused until Accessibility access is granted.` |
| toggle.caption.pausedError | `Paused. The trackpad listener is unavailable.` |
| gesture.chip | `3 fingers` |
| gesture.option.middle | `Middle click` |
| gesture.option.left | `Left click — soon` |
| gesture.option.right | `Right click — soon` |
| gesture.option.mission | `Mission Control — soon` |
| gesture.caption | `Quick tap, then lift` |
| tune.title | `Fine-tuning` |
| tune.reset | `Reset to defaults` |
| tune.duration.label | `Tap window` |
| tune.duration.value | `{n} ms` |
| tune.movement.label | `Movement tolerance` |
| tune.movement.value | `{n}%` |
| perm.title | `Accessibility access needed` |
| perm.body | `Trio needs it to send middle clicks.` |
| perm.open | `Open Settings` |
| perm.check | `Check again` |
| err.title | `Trackpad listener unavailable` |
| err.body | `{listener_error}` |
| err.retry | `Retry` |
| toast.saved | `✓ Saved` |
| toast.failed | `Couldn't save: {reason}` |
| footer.version | `Trio 0.1.0` |
| footer.quit | `Quit` |
| aria.toggle | `Enable Trio` (via `aria-labelledby` to toggle.label) |
| aria.action | `Action for 3-finger tap` |

Rust `listener_error` strings (exact):
- `MultitouchSupport could not be loaded.`
- `MultitouchSupport is missing required functions.`
- `No trackpad detected.`
- `Gesture listener runs on macOS only.` (stub)

---

## 6. Interaction rules (must not change)

1. The switch applies to the engine **immediately** AND persists in the same command. There is no Save button.
2. Sliders persist on `change` (release or key step), never on `input`. The engine reads the new values on the next tap, without a restart.
3. **No modal dialogs**: no `alert/confirm/prompt`, no `<dialog>`, no native message boxes. The only OS-level prompt is the macOS AX prompt, triggered by an explicit button.
4. Nothing ever needs an app restart: permission grant, toggle, params, and retry all apply live.
5. The UI renders **only** from Rust state. The truth is `status = { enabled, permission, listener, listener_error }`. It arrives via `get_status` on load and the `trio:status` event on every change. Optimistic UI is allowed only for the switch, and must reconcile to the next status.
6. Window: 720×540, `resizable:false`, `maximizable:false`, `center:true`. Closing the window hides it (the app keeps running); a Dock click re-shows it; `Quit` or ⌘Q exits (see OQ-1).
7. The frontend must run without Tauri (plain browser). If `window.__TAURI__` is undefined, `main.js` uses an in-file mock backend (§6.1).

### 6.1 Backend contract (`commands.rs`), `withGlobalTauri: true`, called via `window.__TAURI__.core.invoke`

| Command | Args | Returns | Side effects |
|---|---|---|---|
| `get_status` | — | `Status` | — |
| `get_config` | — | `Config` (schema v1) | — |
| `set_enabled` | `{enabled: bool}` | `Status` | engine on/off, persist, emit |
| `set_gesture_params` | `{max_duration_ms: u32, max_movement: f32}` | `Config` (clamped) | persist, hot-swap |
| `reset_gesture_params` | — | `Config` | persist defaults for those 2 keys |
| `set_action` | `{action: string}` | `Config` | only `middle_click` is accepted, else Err |
| `request_permission` | — | `Status` | AX prompt + open Settings pane |
| `retry_listener` | — | `Status` | stop + start listener, emit |
| `quit` | — | — | `app.exit(0)` |

`Status` JSON: `{"enabled":bool,"permission":"granted"|"missing"|"unsupported","listener":"running"|"stopped"|"error","listener_error":string|null}`.
Errors are returned as `Err(String)`; the UI shows them in the toast.

**Mock (browser) modes:** URL `?mock=ok|off|noperm|error|both|savefail` (default `ok`). `window.__trioMock = { push(partialStatus), calls: [] }`. Every mocked invoke appends `{cmd,args}` to `calls` and logs `[mock] <cmd> <json>`. `savefail` rejects persisting commands with `disk full (mock)`.

---

## 7. Acceptance criteria

**W** = QA can verify on Windows (cargo, file inspection, browser with mock, DevTools emulation). **M** = macOS only (real trackpad, real permission).

| AC | Criterion | Where |
|---|---|---|
| AC-1 | `cargo check` in `src-tauri` exits 0 on Windows. | W |
| AC-2 | `cargo test` exits 0 and includes passing tests for: 3-finger tap fires; 4-finger doesn't; 2-finger doesn't; centroid move > max_movement doesn't; hold > max_duration_ms doesn't; second tap inside cooldown doesn't; resting/stationary 3-finger frames still fire; lifting order irrelevant. | W |
| AC-3 | `tauri.conf.json` window: `width 720`, `height 540`, `resizable false`, `maximizable false`, `center true`, `title "Trio"`; `app.withGlobalTauri true`; `build.frontendDist` points at `../src`. | W |
| AC-4 | Config unit tests: save → load returns equal struct; missing file → defaults; malformed JSON → defaults; out-of-range values clamp to §4.4 ranges; unknown fields ignored; no `*.tmp` left after save. | W |
| AC-5 | `gesture.rs` contains no `cfg(target_os`, no `std::thread`, and no `crate::platform` reference (grep). | W |
| AC-6 | No link-time dependency on MultitouchSupport: grep finds no `#[link(name = "MultitouchSupport"` and `macos.rs` uses `dlopen`/`dlsym`. `platform/mod.rs` includes `macos` only under `#[cfg(target_os = "macos")]` and `stub` otherwise. | W |
| AC-7 | `src/index.html` opened in a browser at a 720×540 viewport: no scrollbar and no clipped element in each mock mode `ok, off, noperm, error, both`. | W |
| AC-8 | Every visible string equals §5 (QA diff of DOM text vs table); no other visible text. | W |
| AC-9 | Select shows `Middle click` selected; the other 3 options carry `disabled`. | W |
| AC-10 | Mock `ok`: clicking the switch flips `aria-checked`, the pill (`Active`↔`Off`) and the caption, and logs exactly one `set_enabled` call per click. | W |
| AC-11 | Dragging a slider updates its value label continuously; `set_gesture_params` is logged once per release, not per `input`. | W |
| AC-12 | Mock `noperm`: the permission banner shows with §5 copy, the switch shows paused (opacity 0.45), the pill reads `Needs access`. Mock `error`/`both`: only the danger banner shows, with the reason text, and the pill reads `Unavailable`. | W |
| AC-13 | `__trioMock.push({permission:"granted"})` in mock `noperm` removes the banner and sets the pill to `Active` without a reload. | W |
| AC-14 | After a persisting action, `✓ Saved` appears and is gone after ≈1.8s (1600 + fade). Mock `savefail` shows `Couldn't save: disk full (mock)` and reverts the switch. | W |
| AC-15 | Keyboard only: Tab order matches §8; each stop shows the 2px accent ring; Space toggles the switch; arrow keys move sliders and each step logs a persist call. | W |
| AC-16 | DevTools `prefers-color-scheme: dark` swaps every token to the §3.1 dark column (spot-check bg, surface, text, accent-strong). | W |
| AC-17 | DevTools `prefers-reduced-motion: reduce`: computed `transition-duration` is `0.01ms` on the switch, banner, and toast. | W |
| AC-18 | `main.js` contains no `alert(`, `confirm(`, `prompt(`, or `<dialog`. | W |
| AC-19 | Every interactive element's bounding box is ≥ 28×28 (DevTools). | W |
| AC-20 | README covers the macOS build, first run, the Accessibility grant, and the "Look up → Tap with three fingers" conflict. | W |
| AC-21 | On macOS 13+ the app opens centred at 720×540 content size. Edges don't resize, and the green zoom button is disabled. | M |
| AC-22 | First run without AX trust: the banner shows. `Open Settings` opens Privacy & Security › Accessibility. After the grant, the banner disappears within 3s with no restart. | M |
| AC-23 | 3-finger tap on a link in Safari opens it in a background tab (middle click). Tapping on a Safari tab closes it. Works on the built-in trackpad; on a Magic Trackpad if available. | M |
| AC-24 | 4-finger tap, 2-finger tap, 3-finger swipe/scroll, and pinch emit no middle click (verify with a middle-click test page that logs `auxclick` button 1). | M |
| AC-25 | 3 fingers held > 400ms (default) then lifted: no click. Two quick taps < 250ms apart: one click. | M |
| AC-26 | Switch off → no clicks; switch on → clicks again; no restart. The state survives quit + relaunch. | M |
| AC-27 | Tap window set to 150: a deliberate ~300ms tap no longer fires, without restart. | M |
| AC-28 | Launch with `TRIO_MT_PATH=/nonexistent`: no crash; danger banner `MultitouchSupport could not be loaded.` | M |
| AC-29 | Idle with the listener running: Trio CPU < 1% in Activity Monitor over 60s. | M |

**Split: 20 Windows-verifiable (AC-1…AC-20), 9 macOS-only (AC-21…AC-29).** Every criterion that proves the core job (AC-23…AC-27) is macOS-only. The team can only show it is *likely* to work, through AC-2 and the cross-target check in plan step 6.

---

## 8. Accessibility and keyboard

**Focus order (Tab):** [banner primary `Open Settings` → banner `Check again`] or [`Retry`] (only when a banner shows) → Enable switch → Action select → Tap window slider → Movement tolerance slider → `Reset to defaults` → `Quit`. DOM order equals this order; no positive `tabindex`.
**Keys:** Space/Enter = switch; native select keys; slider ←/→/↑/↓ = 1 step, PageUp/PageDown = 10 steps, Home/End = min/max; ⌘W hides the window; ⌘Q quits. Esc does nothing (there are no overlays).
**ARIA:** switch `role="switch"` + `aria-checked`; sliders have `<label for>` + `aria-valuetext` equal to the visible value (`400 ms`, `8%`); banners `role="alert"` (danger) / `role="status"` (warning); toast `aria-live="polite"`; decorative glyph/arrow/dots `aria-hidden`.
**Hit targets:** switch 44×28, select 220×32, sliders 300×28, all buttons 28 high and ≥ 56 wide, Reset 28 high. The minimum is 28.

**Contrast (WCAG 2.x, ratio = (L1 + 0.05)/(L2 + 0.05), L = relative luminance of sRGB-linearised channels: 0.2126R + 0.7152G + 0.0722B).**

| Pair (fg on bg) | L fg | L bg | Ratio | Need | Pass |
|---|---|---|---|---|---|
| L text #1D1D1F on surface #FFFFFF | 0.0124 | 1.0000 | 16.83 | 4.5 | ✓ |
| L text #1D1D1F on bg #F5F5F7 | 0.0124 | 0.9143 | 15.46 | 4.5 | ✓ |
| L muted #6E6E73 on #FFFFFF | 0.1570 | 1.0000 | 5.07 | 4.5 | ✓ |
| L muted #6E6E73 on #F5F5F7 (footer) | 0.1570 | 0.9143 | 4.66 | 4.5 | ✓ |
| L #FFFFFF on accent-strong #0066CC | 1.0000 | 0.1386 | 5.57 | 4.5 | ✓ |
| L accent-text #0066CC on #FFFFFF | 0.1386 | 1.0000 | 5.57 | 4.5 | ✓ |
| L success #1E7B34 on #F5F5F7 (toast) | 0.1469 | 0.9143 | 4.90 | 4.5 | ✓ |
| L danger #D70015 on #F5F5F7 (toast) | 0.1450 | 0.9143 | 4.94 | 4.5 | ✓ |
| L warning #8A3B00 on warning-bg #FFF4E5 | 0.0853 | 0.9162 | 7.14 | 4.5 | ✓ |
| L text #1D1D1F on #FFF4E5 | 0.0124 | 0.9162 | 15.49 | 4.5 | ✓ |
| L danger #D70015 on danger-bg #FDECEC | 0.1450 | 0.8693 | 4.71 | 4.5 | ✓ |
| L text #1D1D1F on #FDECEC | 0.0124 | 0.8693 | 14.73 | 4.5 | ✓ |
| L accent #0A84FF (switch/focus) vs #FFFFFF | 0.2379 | 1.0000 | 3.65 | 3.0 non-text | ✓ |
| L accent #0A84FF focus ring vs #F5F5F7 | 0.2379 | 0.9143 | 3.35 | 3.0 | ✓ |
| L switch-off-border #8A8A8E vs #FFFFFF | 0.2553 | 1.0000 | 3.44 | 3.0 | ✓ |
| D text #F5F5F7 on surface #2C2C2E | 0.9143 | 0.0253 | 12.80 | 4.5 | ✓ |
| D text #F5F5F7 on bg #1C1C1E | 0.9143 | 0.0117 | 15.63 | 4.5 | ✓ |
| D text #F5F5F7 on raised #3A3A3C | 0.9143 | 0.0425 | 10.42 | 4.5 | ✓ |
| D muted #98989D on #2C2C2E | 0.3157 | 0.0253 | 4.85 | 4.5 | ✓ |
| D muted #98989D on #1C1C1E | 0.3157 | 0.0117 | 5.93 | 4.5 | ✓ |
| D muted #98989D on #3A3A3C | 0.3157 | 0.0425 | 3.95 | 4.5 | ✗ → banned (§3.1) |
| D #FFFFFF on accent-strong #0A6CDB | 1.0000 | 0.1590 | 5.02 | 4.5 | ✓ |
| D accent-text #409CFF on #2C2C2E | 0.3209 | 0.0253 | 4.92 | 4.5 | ✓ |
| D success #30D158 on #1C1C1E (toast) | 0.4693 | 0.0117 | 8.42 | 4.5 | ✓ |
| D danger #FF6961 on #1C1C1E (toast) | 0.3223 | 0.0117 | 6.03 | 4.5 | ✓ |
| D warning #FFD60A on #3D3418 | 0.6938 | 0.0351 | 8.74 | 4.5 | ✓ |
| D text #F5F5F7 on #3D3418 | 0.9143 | 0.0351 | 11.33 | 4.5 | ✓ |
| D danger #FF6961 on #3D1F1E | 0.3223 | 0.0207 | 5.27 | 4.5 | ✓ |
| D text #F5F5F7 on #3D1F1E | 0.9143 | 0.0207 | 13.65 | 4.5 | ✓ |
| D accent #0A84FF vs surface #2C2C2E | 0.2379 | 0.0253 | 3.82 | 3.0 | ✓ |
| D switch-off-border #8E8E93 vs #2C2C2E | 0.2720 | 0.0253 | 4.27 | 3.0 | ✓ |

Worked example (danger on danger-bg): (0.8693 + 0.05) / (0.1450 + 0.05) = 0.9193 / 0.1950 = 4.71.
Rejected during design: white on #0A84FF = 3.65 and white on #007AFF = 4.02 both fail 4.5. That is why filled buttons use `--accent-strong`, not `--accent`. Light #248A3D on white = 4.40 fails, so success is #1E7B34.
Disabled text (`--text-disabled`, light 3.26 on #FFFFFF, dark 2.75 on #2C2C2E) is exempt under WCAG 1.4.3. It is used only on disabled select options.
`--border` (light 1.51, dark 1.53 against surface) is decorative only. No control relies on it alone: the switch-off state has its own 3.44/4.27 outline.

---

## 9. Open questions (recommended default = what Dev implements unless Lead overrides)

1. **Close behaviour.** The charter does not say. **Default:** close hides the window (`on_window_event` CloseRequested → `prevent_close` + `hide`), the macOS `RunEvent::Reopen` re-shows it, and `Quit`/⌘Q exits. A tray icon is deferred to v0.2.
2. **`launch_at_login` has no UI.** **Default:** keep the field, ignore it, don't add the autostart plugin in the MVP.
3. **`build.rs` is not in the charter layout,** but Tauri 2 `generate_context!` needs `tauri_build::build()`. **Default:** add `src-tauri/build.rs` (one line). Lead to bless.
4. **Contact "down" filter** in MultitouchSupport frames (reverse-engineered `state` field). **Default:** count contacts with `state` 4 (make-touch) or 5 (touching) as down, and ignore hover and break/linger states. Behind `TRIO_DEBUG=1`, log per-frame `count/state` to stderr so the first Mac tester can tune it.
5. **Dock icon.** A "menu-bar-style" app would normally hide it (`LSUIElement`), but without a tray that makes the window unreachable. **Default:** keep the Dock icon in the MVP (regular activation policy).
