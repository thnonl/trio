# App inventory — what actually exists

Owner: QA. Baseline for every parity claim in `test-cases.md` / `verify-report.md`.
Every value below was read from the file named, or measured in Chromium at a 720×540
viewport (see `verify-report.md` §2 for the harness).

## 0. Snapshot this describes

The implementer was still writing while this review started, so the inventory is pinned
to one snapshot. Files changed three times under me (macOS file 16:43:04 → 16:43:12 →
16:45:34); everything below is the **16:45:34 snapshot**, which was stable for the rest
of the session.

| File | mtime | bytes | SHA-256 (first 12) |
|---|---|---|---|
| `src/index.html` | 16:43:37 | 5807 | E0135A8D94C8 |
| `src/styles.css` | 16:43:45 | 14196 | B08E9BE1A539 |
| `src/main.js` | 16:44:22 | 17726 | 74ED075CE4C2 |
| `src-tauri/src/lib.rs` | 16:42:34 | 7844 | 65590C2829C5 |
| `src-tauri/src/commands.rs` | 16:43:18 | 4942 | 7E951B9EFF97 |
| `src-tauri/src/config.rs` | 16:41:40 | 12449 | 40FB1E00A558 |
| `src-tauri/src/gesture.rs` | 16:41:40 | 15142 | 195D5A2BEE53 |
| `src-tauri/src/platform/macos.rs` | 16:45:34 | 22872 | (re-read after 16:45) |
| `src-tauri/src/platform/mod.rs` | 16:44:43 | 1422 | — |
| `src-tauri/src/platform/stub.rs` | 16:43:04 | 697 | 5C516B051E83 |
| `src-tauri/src/main.rs` | 16:15:31 | 168 | EA59BF6CCE04 |
| `src-tauri/tauri.conf.json` | — | 1180 | — |
| `src-tauri/capabilities/default.json` | — | 300 | — |

---

## 1. Screens and regions

One window, one screen, no navigation, no overlays. DOM root:

```
body (padding 24px 32px 20px, overflow hidden, flex centre)
└─ main.app                     656 × 496 (measured)
   ├─ header.header             44 high: glyph 32×32 · (h1 "Trio" + p.subtitle) · status pill
   ├─ div.banner-slot           0 high when both banners hidden; 52 when one shows
   │  ├─ #error-banner          danger banner (listener error) — takes the slot first
   │  └─ #permission-banner     warning banner
   ├─ section.card              656 × 285 (measured; ONE card, not the spec's two)
   │  ├─ h2#card-heading        sr-only, never painted
   │  ├─ div.row--toggle        label + caption | switch
   │  ├─ hr.divider
   │  ├─ div.row--gesture       chip · arrow · select | caption
   │  ├─ hr.divider
   │  └─ div.tune               title row (h3 + Reset) + two sliders
   ├─ div.spacer                flex 1, min 16 — 75 px with no banner, 23 px with one
   └─ footer.footer             28 high: "Trio 0.1.0" · #toast · Quit
```

Measurements (Chromium, 720×540 viewport, all six mock modes):
`html.scrollWidth×scrollHeight = 720×540` = `clientWidth×clientHeight` in every mode →
**no scrollbar, no clipping**. `app = 656×496`, `card = 656×285`, `spacer = 75 / 23`.

Divergence from spec §2: the spec's two cards (153 + 16 gap + 116 = 285) are rendered as
**one 656×285 card** with two `hr.divider`s. The total vertical budget is therefore
correct; the "two cards" anatomy is not. See `verify-report.md` D5.

---

## 2. Controls

Hit boxes measured in the browser (`getBoundingClientRect`), modal state taken from the
`?mock=` harness.

| # | id | element | visible label | measured box | states | handler → command |
|---|---|---|---|---|---|---|
| 1 | `open-settings` | `<button class="btn btn--primary">` | `Open Settings` | 108×28 | hover brightness 1.08, active 0.92, focus ring | `request_permission` |
| 2 | `check-again` | `<button class="btn btn--secondary">` | `Check again` | 98×28 | as above, secondary | `get_status` |
| 3 | `retry` | `<button class="btn btn--secondary">` | `Retry` | 57×28 | disabled while its own invoke is in flight | `retry_listener` |
| 4 | `toggle` | `<button role="switch" aria-checked>` | `Enable Trio` + caption | **44×28** (track 38×22, knob 18) | off / on / paused (`data-paused`, track opacity .45) / hover `#D1D1D6` `#545456` / focus-visible / disabled before first status | `set_enabled` (optimistic, reconciled from returned `Status`) |
| 5 | `action` | `<select>` | `Middle click` | **220×32** | 1 enabled + 3 disabled options; disabled entirely before first status | `set_action` on `change` |
| 6 | `duration` | `<input type=range>` | `Tap window` / `400 ms` | **295×28** | 150–800 step 10; `--fill` painted; focus ring on thumb | `input` → label only; `change` → `set_gesture_params` |
| 7 | `movement` | `<input type=range>` | `Movement tolerance` / `8%` | **295×28** | 0.02–0.20 step 0.01 | same as above |
| 8 | `reset` | `<button class="link-btn">` | `Reset to defaults` | 102×28 | hover underline, active opacity .7 | `reset_gesture_params` |
| 9 | `quit` | `<button class="btn btn--ghost">` | `Quit` | 56×28 | secondary-ghost | `quit` |

Non-interactive: `span.app-glyph` (3 dots), `span.chip` (`3 fingers` + 3 dots),
`span.gesture-arrow` (`→`), `p.gesture-caption`, `span#pill-text` + `span.pill-dot`,
`span#error-text`, `span#toast`.

Every measured box is ≥ 28 in both axes (AC-19). The two sliders are **295 px**, not the
spec's 300 (see `verify-report.md` D4).

DOM order of focusable controls (verified by regex over `index.html`, matches the visual
order): `retry > open-settings > check-again > toggle > action > duration > movement >
reset > quit`. Spec §8 order is: banner buttons → toggle → select → duration → movement →
reset → quit. The banner buttons appear first in the DOM but are `hidden` unless a banner
shows, so the effective tab order matches.

---

## 3. Visible strings (verbatim, read from the DOM after boot)

`ok` mode, text nodes actually painted:

```
Trio · Three-finger tap → middle click · Active
Enable Trio · Tap with three fingers anywhere on the trackpad.
3 fingers · → · Middle click · Left click — soon · Right click — soon · Mission Control — soon
Quick tap, then lift · Fine-tuning · Tap window · 400 ms · Movement tolerance · 8%
Reset to defaults · Trio 0.1.0 · Quit
```

`noperm` adds `Accessibility access needed`, `Trio needs it to send middle clicks.`,
`Open Settings`, `Check again`, pill `Needs access`, caption
`Paused until Accessibility access is granted.`

`error` / `both` add `Trackpad listener unavailable`, the reason
`MultitouchSupport could not be loaded.`, `Retry`, pill `Unavailable`, caption
`Paused. The trackpad listener is unavailable.` — and the permission banner is not shown
(one banner at most; error wins).

`off` adds pill `Off`, caption `Off. Your trackpad works as usual.`

Dynamic copy: `{n} ms`, `{round(v*100)}%`, `✓ Saved` (written as `"\u2713 Saved"` in
`main.js:207`), `Couldn't save: {reason}` (`main.js:218`).

All 31 spec §5 entries are present verbatim (machine check, `verify-report.md` F2). No
extra visible string: the browser-only `Preview mode` chip that existed in the 16:26
revision is **gone** (`index.html` no longer contains `preview-chip`).

---

## 4. CSS tokens — spec §3.1 vs `src/styles.css`

19 tokens × 2 themes, compared programmatically against the spec table.

| token | light (css) | spec | dark (css) | spec |
|---|---|---|---|---|
| `--bg` | `#f5f5f7` | `#F5F5F7` | `#1c1c1e` | `#1C1C1E` |
| `--surface` | `#ffffff` | `#FFFFFF` | `#2c2c2e` | `#2C2C2E` |
| `--surface-raised` | `#ffffff` | `#FFFFFF` | `#3a3a3c` | `#3A3A3C` |
| `--chip-bg` | `#f5f5f7` | `#F5F5F7` | `#3a3a3c` | `#3A3A3C` |
| `--border` | `#d2d2d7` | `#D2D2D7` | `#48484a` | `#48484A` |
| `--text` | `#1d1d1f` | `#1D1D1F` | `#f5f5f7` | `#F5F5F7` |
| `--text-muted` | `#6e6e73` | `#6E6E73` | `#98989d` | `#98989D` |
| `--text-disabled` | `#8e8e93` | `#8E8E93` | `#6e6e73` | `#6E6E73` |
| `--accent` | `#0a84ff` | `#0A84FF` | `#0a84ff` | `#0A84FF` |
| `--accent-strong` | `#0066cc` | `#0066CC` | `#0a6cdb` | `#0A6CDB` |
| `--accent-text` | `#0066cc` | `#0066CC` | `#409cff` | `#409CFF` |
| `--accent-contrast` | `#ffffff` | `#FFFFFF` | `#ffffff` | `#FFFFFF` |
| `--switch-off` | `#e5e5ea` | `#E5E5EA` | `#48484a` | `#48484A` |
| `--switch-off-border` | `#8a8a8e` | `#8A8A8E` | `#8e8e93` | `#8E8E93` |
| `--success` | `#1e7b34` | `#1E7B34` | `#30d158` | `#30D158` |
| `--warning` | `#8a3b00` | `#8A3B00` | `#ffd60a` | `#FFD60A` |
| `--warning-bg` | `#fff4e5` | `#FFF4E5` | `#3d3418` | `#3D3418` |
| `--danger` | `#d70015` | `#D70015` | `#ff6961` | `#FF6961` |
| `--danger-bg` | `#fdecec` | `#FDECEC` | `#3d1f1e` | `#3D1F1E` |

**Zero divergences.** Case differs only. Every spec value is matched exactly.

Notes:
- `--text-disabled` is **declared but never used** — no rule reads it. Harmless (disabled
  `<option>`s are painted by WebKit), but it is dead weight against AC-8's spirit.
- Non-token values outside the spec spacing scale: `9px` banner padding (spec-sanctioned),
  `-6px` slider-thumb margin (geometric necessity for a 16 px thumb on a 4 px track),
  `10px` select padding (spec value is `0 28 0 10` — matches), `2px` `link-btn` padding.
- Card shadow, raised shadow, knob shadow and the chevron data-URI are exactly the spec
  recipes; dark theme sets `--shadow-card: none`.

---

## 5. Rust command surface

`lib.rs` registers the handler **twice** (lines 177 and 201, identical `generate_handler!`
blocks). Tauri 2 overwrites the field (`tauri-2.12.0/src/app.rs:1723`), so the effect is
the second list; the first is dead code (D3). Both lists are identical, so the surface is:

| command | in spec §6.1? | args | returns | notes |
|---|---|---|---|---|
| `get_config` | yes | — | `Config` | |
| `get_status` | yes | — | `Status` | |
| `set_enabled` | yes | `{enabled: bool}` | `Status` | early-returns when unchanged; persists; starts/stops engine |
| `set_gesture_params` | yes | `{max_duration_ms, max_movement}` | `Config` | clamps via `Config::sanitize` |
| `reset_gesture_params` | yes | — | `Config` | 400 / 0.08 |
| `set_action` | yes | `{action}` | `Config` | only `middle_click`, else `Err` |
| `request_permission` | yes | — | `Status` | opens the AX pane; **no** C-API prompt (D7) |
| `retry_listener` | yes | — | `Status` | alias of `restart_listener` |
| `quit` | yes | — | — | `app.exit(0)` |
| `set_gesture_param` | **extra** | `{key, value}` | `Config` | single-key variant; not used by the UI |
| `open_accessibility_settings` | **extra** | — | `()` | not used by the UI |
| `restart_listener` | **extra** | — | `Status` | canonical impl behind `retry_listener` |

UI actually invokes exactly the spec's nine; every one has a mock case, and every mock
case exists in Rust (machine check, F4). No command is invoked that is not registered.

## 6. Status contract

`Status` (`lib.rs:30-38`) is now **spec-shaped**:

```json
{"enabled": true, "permission": "granted|missing|unsupported",
 "listener": "running|stopped|error", "listener_error": null|"…"}
```

Event name `trio:status` (`lib.rs:74`), consumed by `main.js:341`. Config schema is the
frozen v1 with `i64`/`f32` numerics so out-of-range values clamp instead of failing
deserialization. The 16:26 frontend revision still read a *boolean* `permission` and an
object `listener`; that mismatch is fixed in this snapshot — the mock and the real backend
now agree.

## 7. Divergences, spec vs app (both values)

| # | Spec | App | Where | Impact |
|---|---|---|---|---|
| 1 | two cards 153 + 16 + 116 | one card 285 tall, two dividers | `index.html:61-147`, `styles.css:285` | visual only; total height identical |
| 2 | sliders 300 px each | 295 px each | `styles.css:538-542` (padding 20) | visual only |
| 3 | spacer 123 / 59 | 75 / 23 | `styles.css:92-99` (`gap: 16` on every child) | visual only; still ≥16, no scroll |
| 4 | select never disabled | disabled before first status | `main.js:131` | ≤1 frame in Tauri |
| 5 | `request_permission` calls AX prompt variant, then opens pane | opens pane only | `commands.rs:148-154` | user sees one prompt instead of two |
| 6 | cooldown clamped 100–1000 | config 100–1000, `Params` 0–2000 | `config.rs:21-22`, `gesture.rs:92` | unreachable via config; test-only |
| 7 | 9 commands | 12 registered (3 aliases) | `lib.rs:177-214` | additive, harmless |
