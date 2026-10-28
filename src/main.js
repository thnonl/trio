/* Trio UI — plain JS, no bundler.
 *
 * Tauri window: window.__TAURI__.core.invoke + the "trio:status" event.
 * Plain browser: an in-page mock with the same command names/shapes, selected
 * with ?mock=ok|off|noperm|error|both|savefail (a review tool, not a feature).
 *
 * The status record is the single source of truth for UI state; only the
 * enable switch updates optimistically, and it reconciles from the command's
 * returned status (or reverts on error).
 */
(() => {
  "use strict";

  const DEFAULTS = {
    version: 1,
    enabled: true,
    gesture: {
      finger_count: 3,
      action: "middle_click",
      max_duration_ms: 400,
      max_movement: 0.08,
      cooldown_ms: 250,
    },
    launch_at_login: false,
  };

  // Spec §4.4 slider ranges; Rust clamps the same bounds.
  const UI_RANGE = {
    max_duration_ms: { min: 150, max: 800 },
    max_movement: { min: 0.02, max: 0.2 },
  };

  const CLAMP = {
    finger_count: [2, 5],
    max_duration_ms: [150, 800],
    max_movement: [0.02, 0.2],
    cooldown_ms: [100, 1000],
  };

  const tauri = window.__TAURI__;
  const PREVIEW = !(tauri && tauri.core && typeof tauri.core.invoke === "function");
  const invoke = PREVIEW ? createPreviewBackend() : tauri.core.invoke;
  const listen =
    !PREVIEW && tauri.event && typeof tauri.event.listen === "function"
      ? tauri.event.listen
      : null;

  const el = (id) => document.getElementById(id);

  const ui = {
    pill: el("status-pill"),
    pillText: el("pill-text"),
    toggle: el("toggle"),
    toggleCaption: el("toggle-caption"),
    action: el("action"),
    duration: el("duration"),
    durationValue: el("duration-value"),
    movement: el("movement"),
    movementValue: el("movement-value"),
    reset: el("reset"),
    permissionBanner: el("permission-banner"),
    errorBanner: el("error-banner"),
    errorText: el("error-text"),
    retry: el("retry"),
    openSettings: el("open-settings"),
    checkAgain: el("check-again"),
    quit: el("quit"),
    toast: el("toast"),
  };

  const state = {
    config: clone(DEFAULTS),
    statusReady: false,
    status: {
      enabled: true,
      permission: "granted",
      listener: "stopped",
      listener_error: null,
    },
  };

  // ---------------------------------------------------------------- rendering

  function render() {
    const config = state.config;
    const status = state.status;
    const gesture = config.gesture || DEFAULTS.gesture;
    const enabled = state.statusReady ? Boolean(status.enabled) : true;
    const granted = status.permission === "granted";
    const hasError = status.listener === "error";
    const listenerError = hasError ? status.listener_error || "" : null;

    // Status pill: first matching row wins (spec §4.1).
    let pillState = "starting";
    let pillText = "Starting\u2026";
    if (state.statusReady) {
      if (!enabled) {
        pillState = "off";
        pillText = "Off";
      } else if (hasError) {
        pillState = "unavailable";
        pillText = "Unavailable";
      } else if (!granted) {
        pillState = "needs-access";
        pillText = "Needs access";
      } else if (status.listener === "running") {
        pillState = "active";
        pillText = "Active";
      }
    }
    ui.pill.dataset.state = pillState;
    ui.pillText.textContent = pillText;

    // Switch + caption.
    ui.toggle.setAttribute("aria-checked", String(enabled));
    ui.toggle.disabled = !state.statusReady;
    ui.toggle.dataset.paused = String(enabled && (!granted || hasError));
    ui.toggleCaption.textContent = captionFor({ enabled, granted, hasError });

    // Banners: the listener error owns the slot when present.
    setBanner(ui.errorBanner, hasError);
    if (hasError) {
      ui.errorText.textContent = listenerError;
      ui.errorText.title = listenerError;
      ui.errorBanner.title = listenerError;
    }
    setBanner(ui.permissionBanner, !hasError && !granted);

    // Action row + sliders.
    ui.action.value = gesture.action;
    ui.action.disabled = !state.statusReady;

    const duration = clamp(
      Number(gesture.max_duration_ms),
      UI_RANGE.max_duration_ms.min,
      UI_RANGE.max_duration_ms.max,
    );
    const movement = clamp(
      Number(gesture.max_movement),
      UI_RANGE.max_movement.min,
      UI_RANGE.max_movement.max,
    );

    if (document.activeElement !== ui.duration) ui.duration.value = String(duration);
    if (document.activeElement !== ui.movement) ui.movement.value = String(movement);

    ui.duration.disabled = !state.statusReady;
    ui.movement.disabled = !state.statusReady;
    ui.reset.disabled = !state.statusReady;

    paintSlider(ui.duration, `${Math.round(duration)} ms`);
    paintSlider(ui.movement, `${Math.round(movement * 100)}%`);
  }

  function captionFor({ enabled, granted, hasError }) {
    if (!enabled) return "Off. Your trackpad works as usual.";
    if (hasError) return "Paused. The trackpad listener is unavailable.";
    if (!granted) return "Paused until Accessibility access is granted.";
    return "Tap with three fingers anywhere on the trackpad.";
  }

  // Banner enter/exit per spec §3.4 (200 ms in, 120 ms out).
  const bannerState = new WeakMap();

  function setBanner(element, visible) {
    const current = bannerState.get(element) || { visible: false, timer: null };
    if (visible) {
      window.clearTimeout(current.timer);
      if (!current.visible) {
        element.hidden = false;
        element.classList.remove("is-leaving");
        element.classList.remove("is-entering");
        void element.offsetWidth; // restart the animation
        element.classList.add("is-entering");
        current.visible = true;
      }
    } else if (current.visible) {
      window.clearTimeout(current.timer);
      element.classList.remove("is-entering");
      element.classList.add("is-leaving");
      current.timer = window.setTimeout(() => {
        element.hidden = true;
        element.classList.remove("is-leaving");
      }, 120);
      current.visible = false;
    }
    bannerState.set(element, current);
  }

  function paintSlider(input, label) {
    const min = Number(input.min);
    const max = Number(input.max);
    const value = Number(input.value);
    const percent = max > min ? ((value - min) / (max - min)) * 100 : 0;
    input.style.setProperty("--fill", `${percent}%`);
    input.setAttribute("aria-valuetext", label);

    const output = input === ui.duration ? ui.durationValue : ui.movementValue;
    output.textContent = label;
  }

  // ------------------------------------------------------------------ feedback

  let toastTimer = null;

  function showSaved() {
    ui.toast.textContent = "\u2713 Saved";
    ui.toast.dataset.tone = "success";
    ui.toast.classList.add("is-visible");
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => {
      ui.toast.classList.remove("is-visible");
    }, 1600);
  }

  function showSaveError(reason) {
    window.clearTimeout(toastTimer);
    ui.toast.textContent = `Couldn't save: ${reason}`;
    ui.toast.dataset.tone = "error";
    ui.toast.classList.add("is-visible");
  }

  function errorText(err) {
    if (typeof err === "string") return err;
    if (err && typeof err.message === "string") return err.message;
    return "unknown error";
  }

  // ------------------------------------------------------------------ handlers

  ui.toggle.addEventListener("click", async () => {
    if (!state.statusReady) return;
    const next = ui.toggle.getAttribute("aria-checked") !== "true";
    ui.toggle.setAttribute("aria-checked", String(next));
    try {
      state.status = await invoke("set_enabled", { enabled: next });
      state.config.enabled = state.status.enabled;
      state.statusReady = true;
      showSaved();
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  });

  function bindSlider(input) {
    input.addEventListener("input", () => {
      if (document.activeElement !== input) return;
      const label =
        input === ui.duration
          ? `${Math.round(Number(input.value))} ms`
          : `${Math.round(Number(input.value) * 100)}%`;
      paintSlider(input, label);
    });

    input.addEventListener("change", () => {
      persistGestureParams();
    });
  }

  async function persistGestureParams() {
    try {
      state.config = await invoke("set_gesture_params", {
        max_duration_ms: Number(ui.duration.value),
        max_movement: Number(ui.movement.value),
      });
      showSaved();
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  }

  bindSlider(ui.duration);
  bindSlider(ui.movement);

  ui.reset.addEventListener("click", async () => {
    try {
      state.config = await invoke("reset_gesture_params", {});
      showSaved();
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  });

  ui.action.addEventListener("change", async () => {
    try {
      state.config = await invoke("set_action", { action: ui.action.value });
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  });

  ui.retry.addEventListener("click", async () => {
    ui.retry.disabled = true;
    try {
      state.status = await invoke("retry_listener", {});
      state.statusReady = true;
    } catch (err) {
      showSaveError(errorText(err));
    } finally {
      ui.retry.disabled = false;
    }
    render();
  });

  ui.openSettings.addEventListener("click", async () => {
    ui.openSettings.disabled = true;
    try {
      state.status = await invoke("request_permission", {});
      state.statusReady = true;
    } catch (err) {
      showSaveError(errorText(err));
    } finally {
      ui.openSettings.disabled = false;
    }
    render();
  });

  ui.checkAgain.addEventListener("click", async () => {
    try {
      state.status = await invoke("get_status");
      state.statusReady = true;
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  });

  ui.quit.addEventListener("click", async () => {
    try {
      await invoke("quit", {});
    } catch (err) {
      showSaveError(errorText(err));
    }
  });

  if (listen) {
    listen("trio:status", (event) => {
      if (event && event.payload) {
        state.status = event.payload;
        state.config.enabled = state.status.enabled;
        state.statusReady = true;
        render();
      }
    }).catch((err) => console.warn("[trio] status listener failed", err));
  }

  window.addEventListener("focus", async () => {
    try {
      state.status = await invoke("get_status");
      state.statusReady = true;
      render();
    } catch (err) {
      /* window is probably closing */
    }
  });

  // -------------------------------------------------------------------- boot

  async function boot() {
    if (PREVIEW) console.info("[trio] preview mode: in-page mock backend");
    try {
      const [config, status] = await Promise.all([invoke("get_config"), invoke("get_status")]);
      state.config = config;
      state.status = status;
      state.statusReady = true;
    } catch (err) {
      showSaveError(errorText(err));
    }
    render();
  }

  boot();

  // --------------------------------------------------------- preview backend

  function createPreviewBackend() {
    const KEY = "trio.preview.config.v1";
    const params = new URLSearchParams(window.location.search);
    const mode = params.get("mock") || "ok";

    let config = readStored();
    let permission = mode === "noperm" || mode === "both" ? "missing" : "granted";
    let listener = mode === "error" || mode === "both" ? "error" : "running";
    let listenerError =
      listener === "error" ? "MultitouchSupport could not be loaded." : null;

    if (mode === "off") config.enabled = false;

    const calls = [];
    window.__trioMock = {
      calls,
      push(partial) {
        applyPush(partial || {});
        state.status = buildStatus();
        state.config.enabled = state.status.enabled;
        state.statusReady = true;
        render();
      },
    };

    function readStored() {
      try {
        const raw = window.localStorage.getItem(KEY);
        if (!raw) return clone(DEFAULTS);
        return sanitize(JSON.parse(raw));
      } catch (err) {
        return clone(DEFAULTS);
      }
    }

    function persist() {
      if (mode === "savefail") return;
      try {
        window.localStorage.setItem(KEY, JSON.stringify(config));
      } catch (err) {
        /* file:// in some browsers or private mode: preview stays in memory */
      }
    }

    function buildStatus() {
      return {
        enabled: config.enabled,
        permission,
        listener: config.enabled ? listener : "stopped",
        listener_error: listener === "error" ? listenerError : null,
      };
    }

    function applyPush(partial) {
      if (typeof partial.enabled === "boolean") config.enabled = partial.enabled;

      if (typeof partial.permission === "boolean") {
        permission = partial.permission ? "granted" : "missing";
      } else if (typeof partial.permission === "string") {
        permission = partial.permission;
      }

      if (typeof partial.listener === "string") {
        listener = partial.listener;
        if (listener !== "error") listenerError = null;
      }

      if (typeof partial.listener_error === "string") {
        listenerError = partial.listener_error;
        listener = "error";
      }
    }

    function logCall(cmd, args) {
      calls.push({ cmd, args: args || {} });
      console.log(`[mock] ${cmd} ${JSON.stringify(args || {})}`);
    }

    function failIfSaveMode() {
      if (mode === "savefail") throw new Error("disk full (mock)");
    }

    return async function invokeMock(command, args) {
      const request = args || {};
      logCall(command, request);
      await new Promise((resolve) => window.setTimeout(resolve, 60));

      switch (command) {
        case "get_config":
          return clone(config);

        case "get_status":
          return buildStatus();

        case "set_enabled":
          failIfSaveMode();
          config.enabled = Boolean(request.enabled);
          if (config.enabled) {
            listener = permission === "granted" ? "running" : "stopped";
            listenerError = null;
          }
          persist();
          return buildStatus();

        case "set_gesture_params": {
          failIfSaveMode();
          if (Number.isFinite(request.max_duration_ms)) {
            config.gesture.max_duration_ms = Math.round(request.max_duration_ms);
          }
          if (Number.isFinite(request.max_movement)) {
            config.gesture.max_movement = request.max_movement;
          }
          config = sanitize(config);
          persist();
          return clone(config);
        }

        case "set_gesture_param": {
          failIfSaveMode();
          if (!Number.isFinite(request.value)) {
            throw new Error(`value for ${request.key} must be a finite number`);
          }
          if (!Object.prototype.hasOwnProperty.call(CLAMP, request.key)) {
            throw new Error(`unknown gesture parameter: ${request.key}`);
          }
          config.gesture[request.key] = request.value;
          config = sanitize(config);
          persist();
          return clone(config);
        }

        case "reset_gesture_params":
          failIfSaveMode();
          config.gesture.max_duration_ms = DEFAULTS.gesture.max_duration_ms;
          config.gesture.max_movement = DEFAULTS.gesture.max_movement;
          persist();
          return clone(config);

        case "set_action":
          failIfSaveMode();
          if (request.action !== "middle_click") {
            throw new Error(`unsupported action: ${request.action}`);
          }
          config.gesture.action = "middle_click";
          persist();
          return clone(config);

        case "open_accessibility_settings":
        case "request_permission":
          permission = "granted";
          if (listener !== "error") listener = "running";
          return buildStatus();

        case "restart_listener":
        case "retry_listener":
          listener = permission === "granted" ? "running" : "stopped";
          listenerError = null;
          return buildStatus();

        case "quit":
          console.log("[mock] quit is a no-op in preview mode");
          return null;

        default:
          throw new Error(`unknown command: ${command}`);
      }
    }

    function sanitize(value) {
      const next = clone(DEFAULTS);
      if (!value || typeof value !== "object") return next;

      if (typeof value.enabled === "boolean") next.enabled = value.enabled;
      const gesture = value.gesture;
      if (!gesture || typeof gesture !== "object") return next;

      if (Number.isFinite(gesture.finger_count)) {
        next.gesture.finger_count = clamp(
          Math.round(gesture.finger_count),
          ...CLAMP.finger_count,
        );
      }
      if (Number.isFinite(gesture.max_duration_ms)) {
        next.gesture.max_duration_ms = clamp(
          Math.round(gesture.max_duration_ms),
          ...CLAMP.max_duration_ms,
        );
      }
      if (Number.isFinite(gesture.max_movement)) {
        next.gesture.max_movement = clamp(gesture.max_movement, ...CLAMP.max_movement);
      }
      if (Number.isFinite(gesture.cooldown_ms)) {
        next.gesture.cooldown_ms = clamp(Math.round(gesture.cooldown_ms), ...CLAMP.cooldown_ms);
      }
      return next;
    }
  }

  function clamp(value, min, max) {
    return Math.min(max, Math.max(min, value));
  }

  function clone(value) {
    return JSON.parse(JSON.stringify(value));
  }
})();
