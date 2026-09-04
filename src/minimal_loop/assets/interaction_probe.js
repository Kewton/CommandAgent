const fs = require("fs");
const http = require("http");

const url = process.argv[2];
const outputPath = process.argv[3];
const probeOptions = (() => {
  try {
    return JSON.parse(process.argv[4] || "{}");
  } catch (_) {
    return {};
  }
})();
const persistenceRequired = !!probeOptions.persistence_required;
const textEntryRequired = !!probeOptions.text_entry_required;
const tokenEchoRequired = !!probeOptions.token_echo_required;
const started = Date.now();
const LAUNCH_TIMEOUT_MS = 20000;
const GOTO_TIMEOUT_MS = 12000;
const WARMUP_GOTO_TIMEOUT_MS = 45000;
const WARMUP_RETRY_DELAY_MS = 2000;
const SERVER_CHECK_TIMEOUT_MS = 5000;
const RENDER_POLL_INTERVAL_MS = 250;
const MARKER_POLL_INTERVAL_MS = 80;
const RENDER_SETTLE_MS = 120;
const HELD_INPUT_OBSERVE_MS = 320;
const TOKEN_ECHO_SETTLE_MS = 3000;
const PERSISTENCE_SURFACE_SETTLE_MS = 3000;
const RELOAD_RENDER_SETTLE_MS = 3000;
const steps = [];
let stage = "resolving";
let before_marker = "";
let after_marker = "";
let input_before_marker = "";
let input_typed_marker = "";
let input_after_marker = "";
let recovery_before_marker = "";
let recovery_after_marker = "";
let recovery_transition_status = "unknown";
let probe_mode = "heuristic";
let contract_hook_status = "unknown";
let contract_hooks = null;
let primary_transition_observed = false;
let start_control_found = true;
let input_state_evaluated_after_start = false;
let candidate_table = [];
let input_dispatches = [];
let canvas_snapshots = [];
let state_dimensions_changed = [];
let surface_fit = null;
let restart_hook_reachable_after_start = false;
let restart_hook_count_after_start = 0;
let persistence_after_reload = "not_evaluated";
let persistence_after_reload_reason = "";
let persistence_changed_dimensions = [];
let persistence_before_reload_marker = "";
let persistence_after_reload_marker = "";
let action_hooks = [];
let text_entry = "not_evaluated";
let text_entry_target = "";
let typed_token = `anvil-${Math.random().toString(36).slice(2, 8) || "probe"}`;
let token_echoed = false;
let echo_latency_ms = null;
let token_echoed_after_reload = false;
let token_echo_after_reload_latency_ms = null;
let text_input_state_change = false;
let informational_failure_kinds = [];
let http_mutation_observations = [];
let http_mutation_failure = "";
let server_check = { ok: false, status: null, error: "" };
let post_js_surface = null;
let cold_start_ms = null;
let measured_navigation_ms = null;
let warmup_attempts = 0;

function write(value) {
  fs.mkdirSync(require("path").dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, JSON.stringify(value, null, 2) + "\n");
}

function writeFailure(value) {
  write(value);
  try {
    process.stdout.write(JSON.stringify(value) + "\n");
  } catch (_) {}
}

function mark(nextStage, extra = {}) {
  stage = nextStage;
  try {
    process.stderr.write(JSON.stringify({ stage, ...extra }) + "\n");
  } catch (_) {}
}

function rawHttpGet(targetUrl) {
  return new Promise((resolve) => {
    let parsed;
    try {
      parsed = new URL(targetUrl);
    } catch (err) {
      resolve({ ok: false, status: null, error: err && err.message ? err.message : String(err) });
      return;
    }
    const request = http.get({
      protocol: parsed.protocol,
      hostname: parsed.hostname,
      port: parsed.port,
      path: `${parsed.pathname || "/"}${parsed.search || ""}`,
      timeout: SERVER_CHECK_TIMEOUT_MS,
      headers: {
        "Connection": "close",
        "User-Agent": "commandagent-interaction-probe"
      }
    }, (response) => {
      response.resume();
      response.on("end", () => {
        resolve({ ok: true, status: response.statusCode || 0, error: "" });
      });
    });
    request.on("timeout", () => {
      request.destroy(new Error("server_check_timeout"));
    });
    request.on("error", (err) => {
      const code = err && err.code ? `${err.code}: ` : "";
      resolve({ ok: false, status: null, error: `${code}${err && err.message ? err.message : String(err)}` });
    });
  });
}

async function marker(page) {
  return await page.evaluate(() => {
    const textOf = (el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim();
    const buttons = Array.from(document.querySelectorAll("button,[role=button]"))
      .map((el) => textOf(el))
      .join("|");
    const body = (document.body && document.body.innerText ? document.body.innerText : "")
      .replace(/\s+/g, " ")
      .slice(0, 800);
    const element_count = document.querySelectorAll("*").length;
    const canvases = Array.from(document.querySelectorAll("canvas"))
      .slice(0, 3)
      .map((canvas) => {
        try {
          return canvas.toDataURL("image/png").slice(0, 2048);
        } catch (_) {
          return `${canvas.width}x${canvas.height}:unreadable`;
        }
      });
    return JSON.stringify({ buttons, body, element_count, canvases });
  });
}

async function surfaceSnapshot(page) {
  return await page.evaluate(() => {
    const controls = document.querySelectorAll("button,[role=button],input,select,textarea,a[href]");
    const canvases = document.querySelectorAll("canvas");
    const title = document.title || "";
    return {
      has_canvas: canvases.length > 0,
      canvas_count: canvases.length,
      interactive_control_count: controls.length,
      title_text_excerpt: title.slice(0, 120)
    };
  });
}

async function surfaceFitSnapshot(page) {
  return await page.evaluate(() => {
    const visibleElement = (el) => {
      if (!el) return false;
      const style = window.getComputedStyle(el);
      const rect = el.getBoundingClientRect();
      return style.display !== "none"
        && style.visibility !== "hidden"
        && rect.width > 0
        && rect.height > 0;
    };
    const selectors = [
      ["canvas", "canvas"],
      ['[data-anvil-state]', "state"],
      ['[data-anvil-action="primary"]', "primary"],
      ["button,[role=button],input,select,textarea,[contenteditable='true'],[data-anvil-action]", "interactive"]
    ];
    let selected = null;
    let surface = "";
    for (const [selector, label] of selectors) {
      const candidate = Array.from(document.querySelectorAll(selector)).find(visibleElement);
      if (candidate) {
        selected = candidate;
        surface = label === "canvas" ? "canvas" : `${candidate.tagName.toLowerCase()}:${label}`;
        break;
      }
    }
    if (!selected) return null;
    const rect = selected.getBoundingClientRect();
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
    const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
    const overflowLeft = Math.max(0, Math.ceil(-rect.left));
    const overflowTop = Math.max(0, Math.ceil(-rect.top));
    const overflowRight = Math.max(0, Math.ceil(rect.right - viewportWidth));
    const overflowBottom = Math.max(0, Math.ceil(rect.bottom - viewportHeight));
    return {
      surface,
      fits_viewport: overflowLeft === 0 && overflowTop === 0 && overflowRight === 0 && overflowBottom === 0,
      overflow_top_px: overflowTop,
      overflow_right_px: overflowRight,
      overflow_bottom_px: overflowBottom,
      overflow_left_px: overflowLeft,
      viewport_width_px: Math.round(viewportWidth),
      viewport_height_px: Math.round(viewportHeight),
      rect_width_px: Math.round(rect.width),
      rect_height_px: Math.round(rect.height)
    };
  });
}

async function recordCanvasSnapshot(page, name) {
  try {
    const snapshot = await page.evaluate((snapshotName) => {
      const hashBytes = (bytes) => {
        let hash = 2166136261 >>> 0;
        for (let index = 0; index < bytes.length; index += 1) {
          hash ^= bytes[index];
          hash = Math.imul(hash, 16777619) >>> 0;
        }
        return hash.toString(16).padStart(8, "0");
      };
      const emptyCanvasHash = (width, height) => {
        const empty = document.createElement("canvas");
        empty.width = width;
        empty.height = height;
        const ctx = empty.getContext("2d");
        if (!ctx) return "";
        return hashBytes(ctx.getImageData(0, 0, width, height).data);
      };
      const canvases = Array.from(document.querySelectorAll("canvas"))
        .slice(0, 3)
        .map((canvas, index) => {
          const width = Math.max(1, canvas.width || Math.round(canvas.getBoundingClientRect().width) || 1);
          const height = Math.max(1, canvas.height || Math.round(canvas.getBoundingClientRect().height) || 1);
          try {
            const ctx = canvas.getContext("2d");
            if (!ctx) {
              return { index, width, height, readable: false, canvas_blank: null, pixel_hash: "", empty_hash: "" };
            }
            const data = ctx.getImageData(0, 0, width, height).data;
            let allZero = true;
            let allAlphaZero = true;
            for (let offset = 0; offset < data.length; offset += 4) {
              if (data[offset] !== 0 || data[offset + 1] !== 0 || data[offset + 2] !== 0 || data[offset + 3] !== 0) {
                allZero = false;
              }
              if (data[offset + 3] !== 0) {
                allAlphaZero = false;
              }
              if (!allZero && !allAlphaZero) break;
            }
            const pixelHash = hashBytes(data);
            const emptyHash = emptyCanvasHash(width, height);
            const canvasBlank = allZero || allAlphaZero || (!!emptyHash && pixelHash === emptyHash);
            return {
              index,
              width,
              height,
              readable: true,
              canvas_blank: canvasBlank,
              all_zero_pixels: allZero,
              all_alpha_zero: allAlphaZero,
              pixel_hash: pixelHash,
              empty_hash: emptyHash
            };
          } catch (err) {
            return {
              index,
              width,
              height,
              readable: false,
              canvas_blank: null,
              pixel_hash: "",
              empty_hash: "",
              error: err && err.message ? err.message : String(err)
            };
          }
        });
      const readable = canvases.filter((canvas) => canvas.readable);
      const blankCanvasCount = readable.filter((canvas) => canvas.canvas_blank === true).length;
      return {
        name: snapshotName,
        canvas_count: canvases.length,
        readable_canvas_count: readable.length,
        blank_canvas_count: blankCanvasCount,
        canvas_blank: readable.length > 0 ? blankCanvasCount === readable.length : null,
        canvases,
        pixel_hashes: readable.map((canvas) => canvas.pixel_hash).filter(Boolean)
      };
    }, name);
    const existing = canvas_snapshots.findIndex((item) => item.name === name);
    if (existing >= 0) {
      canvas_snapshots[existing] = snapshot;
    } else {
      canvas_snapshots.push(snapshot);
    }
    return snapshot;
  } catch (err) {
    const snapshot = {
      name,
      canvas_count: 0,
      readable_canvas_count: 0,
      blank_canvas_count: 0,
      canvas_blank: null,
      canvases: [],
      pixel_hashes: [],
      error: err && err.message ? err.message : String(err)
    };
    canvas_snapshots.push(snapshot);
    return snapshot;
  }
}

function canvasBlankForSnapshot(name) {
  const snapshot = canvas_snapshots.find((item) => item.name === name);
  return snapshot ? snapshot.canvas_blank : null;
}

function canvasPixelHashesForSnapshot(name) {
  const snapshot = canvas_snapshots.find((item) => item.name === name);
  return snapshot && snapshot.readable_canvas_count > 0
    ? snapshot.pixel_hashes
    : [];
}

function canvasNotRedrawnAfterStart() {
  const beforeStart = canvasPixelHashesForSnapshot("before_start");
  const afterStart = canvasPixelHashesForSnapshot("after_start");
  const afterInputs = canvasPixelHashesForSnapshot("after_inputs");
  if (beforeStart.length === 0 || afterStart.length === 0 || afterInputs.length === 0) {
    return false;
  }
  const before = JSON.stringify(beforeStart);
  return before === JSON.stringify(afterStart) && before === JSON.stringify(afterInputs);
}

function navigationFailureDetail(err) {
  const message = err && err.message ? err.message : String(err);
  const net = message.match(/net::([A-Z0-9_]+)/);
  if (net) return net[1];
  if (/timeout/i.test(message)) return "timeout";
  if (/page crashed/i.test(message)) return "page_crash";
  if (/target closed/i.test(message)) return "target_closed";
  return "navigation_error";
}

async function warmUpNavigation(page, targetUrl) {
  const phaseStarted = Date.now();
  let lastErr;
  for (let attempt = 1; attempt <= 2; attempt += 1) {
    warmup_attempts = attempt;
    try {
      mark("warmup_navigation", { attempt, timeout_ms: WARMUP_GOTO_TIMEOUT_MS });
      await page.goto(targetUrl, { waitUntil: "domcontentloaded", timeout: WARMUP_GOTO_TIMEOUT_MS });
      cold_start_ms = Date.now() - phaseStarted;
      steps.push(attempt === 1 ? "warmup_navigation" : "warmup_navigation_retry");
      return;
    } catch (err) {
      lastErr = err;
      if (attempt === 1) {
        mark("warmup_navigation_retry", { attempt, error: err && err.message ? err.message : String(err) });
        await page.waitForTimeout(WARMUP_RETRY_DELAY_MS);
      }
    }
  }
  cold_start_ms = Date.now() - phaseStarted;
  const detail = navigationFailureDetail(lastErr);
  const err = new Error(lastErr && lastErr.message ? lastErr.message : String(lastErr));
  err.anvilFailureKind = server_check.ok ? "app_route_unresponsive" : "probe_infrastructure_failed:server_unreachable";
  err.navigationFailureKind = `probe_warmup_navigation_failed:${detail}`;
  throw err;
}

async function measuredNavigation(page, targetUrl) {
  mark("measured_navigation", { timeout_ms: GOTO_TIMEOUT_MS });
  const startedAt = Date.now();
  try {
    await page.reload({ waitUntil: "domcontentloaded", timeout: GOTO_TIMEOUT_MS });
    measured_navigation_ms = Date.now() - startedAt;
    steps.push("measured_navigation");
  } catch (err) {
    measured_navigation_ms = Date.now() - startedAt;
    const detail = navigationFailureDetail(err);
    const wrapped = new Error(err && err.message ? err.message : String(err));
    wrapped.anvilFailureKind = server_check.ok ? "app_route_unstable" : "probe_infrastructure_failed:server_unreachable";
    wrapped.navigationFailureKind = `probe_measured_navigation_failed:${detail}`;
    throw wrapped;
  }
}

async function pollRenderedAssertion(page, read, options = {}) {
  const startedAt = options.startedAt || Date.now();
  const deadlineAt = options.deadlineAt || (Date.now() + (options.timeoutMs || 0));
  const intervalMs = options.intervalMs || RENDER_POLL_INTERVAL_MS;
  const matches = options.matches || ((value) => !!value);
  let lastValue;
  while (true) {
    const value = await read();
    lastValue = value;
    if (matches(value)) {
      return { matched: true, value, latency_ms: Date.now() - startedAt };
    }
    const now = Date.now();
    if (now >= deadlineAt) {
      return { matched: false, value: lastValue, latency_ms: null };
    }
    await page.waitForTimeout(Math.min(intervalMs, Math.max(0, deadlineAt - now)));
  }
}

async function settledRenderedValue(page, read, timeoutMs, intervalMs = RENDER_POLL_INTERVAL_MS) {
  let lastText = null;
  let stableSince = 0;
  const result = await pollRenderedAssertion(page, async () => {
    const value = await read();
    let text;
    try {
      text = JSON.stringify(value);
    } catch (_) {
      text = String(value);
    }
    const now = Date.now();
    if (text === lastText) {
      if (!stableSince) stableSince = now;
    } else {
      lastText = text;
      stableSince = now;
    }
    return { value, stable_for_ms: now - stableSince };
  }, {
    timeoutMs,
    intervalMs,
    matches: (sample) => !!sample && sample.stable_for_ms >= RENDER_SETTLE_MS
  });
  const sample = result.value || {};
  return Object.prototype.hasOwnProperty.call(sample, "value") ? sample.value : "";
}

async function markerAfterChange(page, previous, timeoutMs) {
  const result = await pollRenderedAssertion(page, () => marker(page), {
    timeoutMs,
    intervalMs: MARKER_POLL_INTERVAL_MS,
    matches: (current) => current !== previous
  });
  return result.value;
}

async function contractStateMarker(page) {
  return await page.evaluate(() => {
    const stable = (value) => {
      if (Array.isArray(value)) return value.map(stable);
      if (value && typeof value === "object") {
        return Object.keys(value).sort().reduce((out, key) => {
          out[key] = stable(value[key]);
          return out;
        }, {});
      }
      return value;
    };
    const states = Array.from(document.querySelectorAll("[data-anvil-state]"))
      .map((el, index) => {
        const raw = el.getAttribute("data-anvil-state") || "";
        try {
          return { index, state: stable(JSON.parse(raw)) };
        } catch (_) {
          return null;
        }
      })
      .filter(Boolean);
    return JSON.stringify({ states });
  });
}

function stableStateString(value) {
  if (typeof value === "undefined") return "__anvil_undefined__";
  try {
    return JSON.stringify(value);
  } catch (_) {
    return String(value);
  }
}

function contractStatesFromMarker(markerText) {
  try {
    const parsed = JSON.parse(markerText || "{}");
    if (!parsed || !Array.isArray(parsed.states)) return [];
    return parsed.states
      .map((entry) => entry && entry.state && typeof entry.state === "object" && !Array.isArray(entry.state)
        ? entry.state
        : {})
      .filter(Boolean);
  } catch (_) {
    return [];
  }
}

function changedTopLevelStateKeys(beforeText, afterText) {
  const before = contractStatesFromMarker(beforeText);
  const after = contractStatesFromMarker(afterText);
  const keys = new Set();
  const count = Math.max(before.length, after.length);
  for (let index = 0; index < count; index += 1) {
    const beforeState = before[index] || {};
    const afterState = after[index] || {};
    const names = new Set([...Object.keys(beforeState), ...Object.keys(afterState)]);
    for (const name of names) {
      if (stableStateString(beforeState[name]) !== stableStateString(afterState[name])) {
        keys.add(name);
      }
    }
  }
  return Array.from(keys).sort();
}

function mergeStateDimensionsChanged(keys) {
  for (const key of keys || []) {
    if (key && !state_dimensions_changed.includes(key)) {
      state_dimensions_changed.push(key);
    }
  }
  state_dimensions_changed.sort();
}

function changedStateKeysPreservedAfterReload(keys, beforeReloadText, afterReloadText) {
  const beforeReload = contractStatesFromMarker(beforeReloadText);
  const afterReload = contractStatesFromMarker(afterReloadText);
  const count = Math.max(beforeReload.length, afterReload.length);
  for (const key of keys) {
    let sawKey = false;
    for (let index = 0; index < count; index += 1) {
      const beforeState = beforeReload[index] || {};
      const afterState = afterReload[index] || {};
      if (Object.prototype.hasOwnProperty.call(beforeState, key)) {
        sawKey = true;
      }
      if (stableStateString(beforeState[key]) !== stableStateString(afterState[key])) {
        return false;
      }
    }
    if (!sawKey) return false;
  }
  return keys.length > 0;
}

function setPersistenceNotEvaluatedReason(reason) {
  if (!persistence_after_reload_reason) {
    persistence_after_reload_reason = reason;
  }
}

function observedMutationDimensions() {
  return Array.from(new Set(state_dimensions_changed || []))
    .filter(Boolean)
    .sort();
}

function contractPersistenceMutationKeys(beforeReloadText) {
  if (!input_before_marker || !beforeReloadText) return [];
  const committed = changedTopLevelStateKeys(input_before_marker, beforeReloadText);
  if (!input_typed_marker) return committed;
  const draft = new Set(changedTopLevelStateKeys(input_before_marker, input_typed_marker));
  return committed.filter((key) => !draft.has(key));
}

function evaluatePersistenceAfterReload(mode, beforeReloadText, afterReloadText) {
  if (mode === "contract") {
    const changed = contractPersistenceMutationKeys(beforeReloadText);
    persistence_changed_dimensions = changed;
    if (changed.length === 0) {
      setPersistenceNotEvaluatedReason("no_mutation_observed");
      return "not_evaluated";
    }
    return changedStateKeysPreservedAfterReload(changed, beforeReloadText, afterReloadText)
      ? "preserved"
      : "reset";
  }
  if (!input_before_marker || !input_after_marker || input_before_marker === input_after_marker) {
    const changed = observedMutationDimensions();
    if (changed.length > 0) {
      persistence_changed_dimensions = changed;
      return beforeReloadText === afterReloadText ? "preserved" : "reset";
    }
    persistence_changed_dimensions = [];
    setPersistenceNotEvaluatedReason("no_mutation_observed");
    return "not_evaluated";
  }
  if (beforeReloadText !== input_after_marker) {
    const changed = observedMutationDimensions();
    if (changed.length > 0) {
      persistence_changed_dimensions = changed;
      return beforeReloadText === afterReloadText ? "preserved" : "reset";
    }
    persistence_changed_dimensions = [];
    setPersistenceNotEvaluatedReason("no_mutation_observed");
    return "not_evaluated";
  }
  persistence_changed_dimensions = ["marker"];
  return beforeReloadText === afterReloadText ? "preserved" : "reset";
}

function persistenceMarkerLooksPreserved(mode, beforeReloadText, afterReloadText) {
  if (mode === "contract") {
    const changed = contractPersistenceMutationKeys(beforeReloadText);
    return changed.length > 0
      && changedStateKeysPreservedAfterReload(changed, beforeReloadText, afterReloadText);
  }
  if (!input_before_marker || !input_after_marker || input_before_marker === input_after_marker) {
    const changed = observedMutationDimensions();
    return changed.length > 0 && beforeReloadText === afterReloadText;
  }
  if (beforeReloadText !== input_after_marker) {
    const changed = observedMutationDimensions();
    return changed.length > 0 && beforeReloadText === afterReloadText;
  }
  return beforeReloadText === afterReloadText;
}

async function persistenceMarkerAfterReload(page, mode, beforeReloadText) {
  const result = await pollRenderedAssertion(page, async () => {
    const value = await activeMarker(page, mode);
    return {
      value,
      preserved: persistenceMarkerLooksPreserved(mode, beforeReloadText, value)
    };
  }, {
    timeoutMs: RELOAD_RENDER_SETTLE_MS,
    intervalMs: RENDER_POLL_INTERVAL_MS,
    matches: (sample) => !!sample && sample.preserved
  });
  const sample = result.value || {};
  return Object.prototype.hasOwnProperty.call(sample, "value") ? sample.value : "";
}

async function waitForAnySurface(page) {
  const surface = page.locator("canvas, button, [role=button], input, select, textarea, [contenteditable='true'], [data-anvil-action], [data-anvil-state]").first();
  await surface.waitFor({ timeout: 10000 });
}

async function evaluatePersistenceReload(page, mode) {
  if (!persistenceRequired) return;
  persistence_after_reload_reason = "";
  if (!steps.includes("input_state_change")) {
    steps.push("persistence_reload:not_evaluated");
    persistence_after_reload = "not_evaluated";
    setPersistenceNotEvaluatedReason(
      textEntryRequired && text_entry !== "entered" && !text_entry_target
        ? "no_text_entry_surface"
        : "no_mutation_observed"
    );
    return;
  }
  mark("persistence_reload");
  const typedTokenWasPresent = text_input_state_change && token_echoed;
  persistence_before_reload_marker = await settledRenderedValue(
    page,
    () => activeMarker(page, mode),
    PERSISTENCE_SURFACE_SETTLE_MS
  );
  try {
    await page.reload({ waitUntil: "domcontentloaded", timeout: GOTO_TIMEOUT_MS });
    await waitForAnySurface(page);
    persistence_after_reload_marker = typedTokenWasPresent
      ? await settledRenderedValue(
        page,
        () => activeMarker(page, mode),
        RELOAD_RENDER_SETTLE_MS
      )
      : await persistenceMarkerAfterReload(page, mode, persistence_before_reload_marker);
    let reloadEcho = { matched: false, value: false, latency_ms: null };
    if (!token_echoed) {
      const reloadEchoStartedAt = Date.now();
      reloadEcho = await pollTokenEchoedOutsideTextEntryTarget(
        page,
        typed_token,
        reloadEchoStartedAt,
        reloadEchoStartedAt + RELOAD_RENDER_SETTLE_MS
      );
      if (reloadEcho.matched) {
        token_echoed_after_reload = true;
        token_echo_after_reload_latency_ms = reloadEcho.latency_ms;
        steps.push("token_echoed_after_reload");
      }
    }
    if (typedTokenWasPresent) {
      let typedTokenSurvived = reloadEcho.matched;
      if (!typedTokenSurvived) {
        const tokenSurvivedStartedAt = Date.now();
        const survived = await pollTokenEchoedOutsideTextEntryTarget(
          page,
          typed_token,
          tokenSurvivedStartedAt,
          tokenSurvivedStartedAt + RELOAD_RENDER_SETTLE_MS
        );
        typedTokenSurvived = survived.matched;
      }
      persistence_changed_dimensions = ["typed_token"];
      persistence_after_reload = typedTokenSurvived ? "preserved" : "reset";
      persistence_after_reload_reason = "";
    } else {
      persistence_after_reload = evaluatePersistenceAfterReload(
        mode,
        persistence_before_reload_marker,
        persistence_after_reload_marker
      );
    }
    steps.push(
      persistence_after_reload === "not_evaluated"
        ? "persistence_reload:not_evaluated"
        : "persistence_reload"
    );
  } catch (err) {
    persistence_after_reload = "not_evaluated";
    persistence_after_reload_reason = "reload_failed";
    steps.push("persistence_reload:not_evaluated");
    informational_failure_kinds.push("persistence_reload_not_evaluated");
  }
}

function contractResetWardChange(beforeText, afterText, baselineText) {
  if (!beforeText || !afterText || beforeText === afterText) return false;
  const changed = changedTopLevelStateKeys(beforeText, afterText);
  if (changed.length === 0) return false;
  const before = contractStatesFromMarker(beforeText);
  const after = contractStatesFromMarker(afterText);
  const baseline = contractStatesFromMarker(baselineText);
  const count = Math.max(before.length, after.length, baseline.length);
  for (let index = 0; index < count; index += 1) {
    const beforeState = before[index] || {};
    const afterState = after[index] || {};
    const baselineState = baseline[index] || {};
    for (const key of changed) {
      const beforeValue = beforeState[key];
      const afterValue = afterState[key];
      const baselineValue = baselineState[key];
      if (
        stableStateString(afterValue) === stableStateString(baselineValue) &&
        stableStateString(beforeValue) !== stableStateString(baselineValue)
      ) {
        return true;
      }
      if (
        typeof beforeValue === "number" &&
        typeof afterValue === "number" &&
        typeof baselineValue === "number" &&
        Math.abs(afterValue - baselineValue) < Math.abs(beforeValue - baselineValue)
      ) {
        return true;
      }
    }
  }
  return true;
}

async function contractHookStatus(page) {
  return await page.evaluate(() => {
    const textOf = (el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim();
    const primary = document.querySelector('[data-anvil-action="primary"]');
    const restart = document.querySelector('[data-anvil-action="restart"]');
    const action_hooks = Array.from(document.querySelectorAll("[data-anvil-action]"))
      .map((el) => (el.getAttribute("data-anvil-action") || "").trim())
      .filter(Boolean)
      .filter((value, index, values) => values.indexOf(value) === index)
      .sort();
    const stateEls = Array.from(document.querySelectorAll("[data-anvil-state]"));
    let valid_state_count = 0;
    let invalid_state_count = 0;
    for (const el of stateEls) {
      try {
        JSON.parse(el.getAttribute("data-anvil-state") || "");
        valid_state_count += 1;
      } catch (_) {
        invalid_state_count += 1;
      }
    }
    const primary_present = !!primary;
    const state_present = stateEls.length > 0;
    const usable = primary_present && valid_state_count > 0;
    const status = usable
      ? "usable"
      : !primary_present
        ? "primary_missing"
        : !state_present
          ? "state_missing"
          : "state_invalid";
    return {
      status,
      usable,
      primary_present,
      primary_text_excerpt: primary ? textOf(primary).slice(0, 80) : "",
      restart_present: !!restart,
      restart_text_excerpt: restart ? textOf(restart).slice(0, 80) : "",
      action_hooks,
      state_present,
      state_count: stateEls.length,
      valid_state_count,
      invalid_state_count
    };
  });
}

async function restartHookReachability(page) {
  return await page.evaluate(() => {
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
    const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
    const hooks = Array.from(document.querySelectorAll('[data-anvil-action="restart"]'));
    const visible = hooks.some((el) => {
      const style = window.getComputedStyle(el);
      const rect = el.getBoundingClientRect();
      const disabled = !!el.disabled || el.getAttribute("aria-disabled") === "true";
      return !disabled
        && style.display !== "none"
        && style.visibility !== "hidden"
        && style.pointerEvents !== "none"
        && rect.width > 0
        && rect.height > 0
        && rect.right > 0
        && rect.bottom > 0
        && (!viewportWidth || rect.left < viewportWidth)
        && (!viewportHeight || rect.top < viewportHeight);
    });
    return { reachable: visible, count: hooks.length };
  });
}

async function activeMarker(page, mode) {
  return mode === "contract" ? await contractStateMarker(page) : await marker(page);
}

async function markerAfterActiveChange(page, mode, previous, timeoutMs) {
  const result = await pollRenderedAssertion(page, () => activeMarker(page, mode), {
    timeoutMs,
    intervalMs: MARKER_POLL_INTERVAL_MS,
    matches: (current) => current !== previous
  });
  return result.value;
}

async function controlText(locator) {
  try {
    return await locator.evaluate((el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim());
  } catch (_) {
    return "";
  }
}

async function rankedControlCandidates(page, skipContractPrimary) {
  return await page.locator("button,[role=button]").evaluateAll((els, skipPrimary) => {
    const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 1;
    const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 1;
    const centerX = viewportWidth / 2;
    const centerY = viewportHeight / 2;
    const maxDistance = Math.sqrt(centerX * centerX + centerY * centerY) || 1;
    const textOf = (el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim().replace(/\s+/g, " ");
    const visible = (el, box) => {
      const style = window.getComputedStyle(el);
      return box.width > 0
        && box.height > 0
        && style.visibility !== "hidden"
        && style.display !== "none"
        && Number(style.opacity || "1") > 0;
    };
    return els
      .map((el, index) => {
        const box = el.getBoundingClientRect();
        const text = textOf(el);
        const area = Math.round(box.width * box.height);
        const distance = Math.sqrt(
          Math.pow((box.left + box.width / 2) - centerX, 2) +
          Math.pow((box.top + box.height / 2) - centerY, 2)
        );
        const centrality_milli = Math.round(Math.max(0, 1 - distance / maxDistance) * 1000);
        return {
          index,
          text_excerpt: text.slice(0, 80),
          text_len: text.length,
          text_bucket: text.length >= 2 ? 1 : 0,
          area,
          centrality_milli,
          contract_primary: el.getAttribute("data-anvil-action") === "primary",
          visible: visible(el, box)
        };
      })
      .filter((candidate) => candidate.visible)
      .filter((candidate) => !(skipPrimary && candidate.contract_primary))
      .sort((a, b) =>
        (b.text_bucket - a.text_bucket) ||
        (b.area - a.area) ||
        (b.centrality_milli - a.centrality_milli)
      )
      .slice(0, 4)
      .map((candidate, rank) => ({ ...candidate, rank: rank + 1 }));
  }, skipContractPrimary);
}

async function attemptRankedCandidateTransitions(page, mode, skipContractPrimary) {
  const candidates = await rankedControlCandidates(page, skipContractPrimary);
  for (const candidate of candidates) {
    const entry = {
      rank: candidate.rank,
      index: candidate.index,
      text_excerpt: candidate.text_excerpt,
      area: candidate.area,
      centrality_milli: candidate.centrality_milli,
      changed: false
    };
    const before = await activeMarker(page, mode);
    try {
      await page.locator("button,[role=button]").nth(candidate.index).click({ timeout: 1200 });
      const after = await markerAfterActiveChange(page, mode, before, 800);
      entry.changed = before !== after;
      candidate_table.push(entry);
      if (entry.changed) {
        return { observed: true, before, after, source: "candidate", candidate: entry };
      }
    } catch (_) {
      candidate_table.push(entry);
    }
  }
  return { observed: false, before: "", after: "", source: "", candidate: null };
}

async function textEntryCandidate(page) {
  return await page.locator('textarea, input:not([type]), input[type=""], input[type="text"], input[type="search"], [contenteditable=""], [contenteditable="true"]').evaluateAll((els) => {
    const visible = (el, box) => {
      const style = window.getComputedStyle(el);
      return box.width > 0
        && box.height > 0
        && style.visibility !== "hidden"
        && style.display !== "none"
        && Number(style.opacity || "1") > 0
        && !el.disabled
        && el.getAttribute("aria-hidden") !== "true";
    };
    return els
      .map((el, index) => {
        const box = el.getBoundingClientRect();
        const hook = (el.getAttribute("data-anvil-action") || "").trim();
        const tag = (el.tagName || "").toLowerCase();
        return {
          index,
          tag,
          hook,
          area: Math.round(box.width * box.height),
          visible: visible(el, box)
        };
      })
      .filter((candidate) => candidate.visible)
      .sort((a, b) =>
        ((b.hook === "input") - (a.hook === "input")) ||
        (b.area - a.area)
      )[0] || null;
  });
}

async function clickPrimaryActionIfPresent(page) {
  const primary = page.locator('[data-anvil-action="primary"]').first();
  if (await primary.count()) {
    try {
      await primary.click({ timeout: 1200 });
      return true;
    } catch (_) {}
  }
  const candidates = await rankedControlCandidates(page, false);
  for (const candidate of candidates) {
    try {
      await page.locator("button,[role=button]").nth(candidate.index).click({ timeout: 1200 });
      return true;
    } catch (_) {}
  }
  return false;
}

async function tokenEchoedOutsideTextEntryTargetNow(page, token) {
  if (!token) return false;
  return await page.evaluate((value) => {
    const excluded = document.querySelector('[data-anvil-probe-text-target="1"]');
    const isExcluded = (node) => {
      if (!excluded || !node) return false;
      const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
      return !!element && (element === excluded || excluded.contains(element));
    };
    const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node) {
      if (!isExcluded(node) && (node.nodeValue || "").includes(value)) {
        return true;
      }
      node = walker.nextNode();
    }
    return false;
  }, token);
}

async function pollTokenEchoedOutsideTextEntryTarget(page, token, startedAt, deadlineAt) {
  if (!token) return { matched: false, value: false, latency_ms: null };
  return await pollRenderedAssertion(page, () => tokenEchoedOutsideTextEntryTargetNow(page, token), {
    startedAt,
    deadlineAt,
    intervalMs: RENDER_POLL_INTERVAL_MS,
    matches: (found) => found === true
  });
}

async function attemptTextEntry(page, mode) {
  const candidate = await textEntryCandidate(page);
  if (!candidate) {
    text_entry = "not_applicable";
    steps.push("text_entry:not_applicable");
    return false;
  }
  text_entry = "entered";
  text_entry_target = `${candidate.tag}:${candidate.hook ? `data-anvil-action=${candidate.hook}` : "no-hook"}`;
  const locator = page.locator('textarea, input:not([type]), input[type=""], input[type="text"], input[type="search"], [contenteditable=""], [contenteditable="true"]').nth(candidate.index);
  input_before_marker = await activeMarker(page, mode);
  try {
    await locator.evaluate((el) => {
      el.setAttribute("data-anvil-probe-text-target", "1");
    });
    await locator.focus({ timeout: 1200 });
    await page.keyboard.type(typed_token, { delay: 5 });
    input_typed_marker = await activeMarker(page, mode);
    const primaryClicked = await clickPrimaryActionIfPresent(page);
    if (mode === "contract" && !primaryClicked) {
      informational_failure_kinds.push("primary_action_disabled_after_text_entry");
    }
    const echoStartedAt = Date.now();
    input_after_marker = await markerAfterActiveChange(page, mode, input_before_marker, 800);
    text_input_state_change = input_before_marker !== input_after_marker;
    if (mode === "contract") {
      mergeStateDimensionsChanged(changedTopLevelStateKeys(input_before_marker, input_after_marker));
    }
    const echoResult = await pollTokenEchoedOutsideTextEntryTarget(
      page,
      typed_token,
      echoStartedAt,
      echoStartedAt + TOKEN_ECHO_SETTLE_MS
    );
    token_echoed = echoResult.matched;
    echo_latency_ms = echoResult.matched ? echoResult.latency_ms : null;
    steps.push("text_entry");
    if (text_input_state_change) {
      steps.push("text_input_state_change");
      steps.push("input_state_change");
    }
    steps.push(token_echoed ? "token_echoed" : "token_echo_missing");
    return text_input_state_change;
  } catch (err) {
    text_entry = "failed";
    informational_failure_kinds.push("text_entry_failed");
    steps.push("text_entry:failed");
    return false;
  }
}

async function hasStartLikeControl(page) {
  return await page.locator("button,[role=button],input[type=button],input[type=submit]").evaluateAll((els) => {
    const textOf = (el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim();
    const startPattern = /(start|begin|play|restart|retry|new game|スタート|開始|再開|リスタート)/i;
    return els.some((el) => {
      const box = el.getBoundingClientRect();
      const style = window.getComputedStyle(el);
      const visible = box.width > 0
        && box.height > 0
        && style.visibility !== "hidden"
        && style.display !== "none"
        && Number(style.opacity || "1") > 0;
      return visible && startPattern.test(textOf(el));
    });
  });
}

async function dispatchPostTransitionInputs(page, mode) {
  input_before_marker = await activeMarker(page, mode);
  const canvas = page.locator("canvas").first();
  let clicked = false;
  if (await canvas.count()) {
    const box = await canvas.boundingBox();
    if (box) {
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
      clicked = true;
    }
  }
  if (!clicked) {
    const viewport = page.viewportSize() || { width: 1280, height: 720 };
    await page.mouse.click(viewport.width / 2, viewport.height / 2);
  }
  input_dispatches.push("canvas/center click");
  for (const key of ["ArrowLeft", "ArrowRight", "Space"]) {
    input_dispatches.push(`${key} keydown`);
    const keyBefore = await activeMarker(page, mode);
    let keyAfter = keyBefore;
    try {
      await page.keyboard.down(key);
      steps.push(`${key}_keydown_hold`);
      keyAfter = await markerAfterActiveChange(
        page,
        mode,
        keyBefore,
        HELD_INPUT_OBSERVE_MS
      );
    } finally {
      await page.keyboard.up(key);
    }
    if (keyBefore !== keyAfter) {
      input_after_marker = keyAfter;
      break;
    }
  }
  steps.push("control_input_dispatched");
  steps.push("input_key_hold");
  steps.push("input_state_evaluated_after_start");
  input_state_evaluated_after_start = true;
  if (!input_after_marker || input_after_marker === input_before_marker) {
    input_after_marker = await markerAfterActiveChange(page, mode, input_before_marker, 800);
  }
  if (mode === "contract") {
    mergeStateDimensionsChanged(changedTopLevelStateKeys(input_before_marker, input_after_marker));
  }
  if (input_before_marker !== input_after_marker) {
    steps.push("input_state_change");
  }
}

async function dispatchStartlessTextInput(page, mode) {
  const target = page.locator('input:not([type="hidden"]):not([disabled]), textarea:not([disabled]), [contenteditable="true"]').first();
  if (!(await target.count())) {
    return false;
  }
  input_before_marker = await activeMarker(page, mode);
  try {
    await target.click({ timeout: 1200 });
    const tag = await target.evaluate((el) => el.tagName.toLowerCase());
    if (tag === "input" || tag === "textarea") {
      await target.fill("anvil probe input", { timeout: 1200 });
    } else {
      await page.keyboard.type("anvil probe input");
    }
    input_dispatches.push("direct text input");
    steps.push("control_input_dispatched");
    steps.push("input_state_evaluated_after_start");
    input_state_evaluated_after_start = true;
    input_after_marker = await markerAfterActiveChange(page, mode, input_before_marker, 800);
    if (input_before_marker !== input_after_marker) {
      steps.push("input_state_change");
      return true;
    }
  } catch (_) {}
  return false;
}

async function recoveryCandidateIndex(page, initialStartText) {
  return await page.locator("button,[role=button]").evaluateAll((els, initialText) => {
    const normalizedInitial = (initialText || "").trim();
    const textOf = (el) => (
      el.getAttribute("aria-label") ||
      el.getAttribute("title") ||
      el.textContent ||
      el.value ||
      ""
    ).trim();
    const candidates = els
      .map((el, index) => ({ index, text: textOf(el) }))
      .filter((candidate) => candidate.text.length > 0);
    const changed = candidates.find((candidate) => candidate.text !== normalizedInitial);
    if (changed) return changed.index;
    const rerendered = candidates.find((candidate) => normalizedInitial && candidate.text === normalizedInitial);
    return rerendered ? rerendered.index : -1;
  }, initialStartText);
}

async function attemptRecoveryTransition(page, initialStartText, mode) {
  const deadline = Date.now() + 1200;
  while (Date.now() < deadline) {
    const index = await recoveryCandidateIndex(page, initialStartText);
    if (index >= 0) {
      const candidate = page.locator("button,[role=button]").nth(index);
      recovery_before_marker = await activeMarker(page, mode);
      try {
        await candidate.click({ timeout: 1000 });
        recovery_after_marker = await markerAfterActiveChange(page, mode, recovery_before_marker, 700);
        if (recovery_before_marker !== recovery_after_marker) {
          steps.push("recovery_transition");
          recovery_transition_status = "observed";
          return;
        }
      } catch (_) {}
    }
    await page.waitForTimeout(120);
  }
  steps.push("recovery_transition:not_observed");
  recovery_transition_status = "not_observed";
}

async function attemptContractRecoveryTransition(page, postStartMarker) {
  const deadline = Date.now() + 1200;
  while (Date.now() < deadline) {
    let candidate = page.locator('[data-anvil-action="restart"]').first();
    if (!(await candidate.count())) {
      candidate = page.locator('[data-anvil-action="primary"]').first();
    }
    if (await candidate.count()) {
      recovery_before_marker = await activeMarker(page, "contract");
      try {
        await candidate.click({ timeout: 1000 });
        recovery_after_marker = await markerAfterActiveChange(page, "contract", recovery_before_marker, 700);
        if (contractResetWardChange(recovery_before_marker, recovery_after_marker, postStartMarker)) {
          steps.push("recovery_transition");
          recovery_transition_status = "observed";
          return;
        }
      } catch (_) {}
    }
    recovery_before_marker = await activeMarker(page, "contract");
    try {
      await page.keyboard.press("r");
      recovery_after_marker = await markerAfterActiveChange(page, "contract", recovery_before_marker, 700);
      if (contractResetWardChange(recovery_before_marker, recovery_after_marker, postStartMarker)) {
        steps.push("recovery_transition");
        recovery_transition_status = "observed";
        return;
      }
    } catch (_) {}
    await page.waitForTimeout(120);
  }
  steps.push("recovery_transition:not_observed");
  recovery_transition_status = "not_observed";
}

function interactionFailureKind(transitionObserved, inputEvaluated, inputStateChanged) {
  if (!transitionObserved) return "start_transition_missing";
  if (!inputEvaluated) return "input_state_change_not_evaluated_after_start";
  if (!inputStateChanged) return "input_state_change_missing_after_start";
  return "";
}

(async () => {
  let browser;
  try {
    mark("resolving");
    const { chromium } = require("playwright");
    mark("launching");
    browser = await chromium.launch({ headless: true, timeout: LAUNCH_TIMEOUT_MS });
    const page = await browser.newPage();
    page.on("response", (response) => {
      try {
        const request = response.request();
        const method = String(request.method() || "").toUpperCase();
        if (!["POST", "PUT", "PATCH", "DELETE"].includes(method)) return;
        const observation = {
          method,
          status: response.status(),
          ok: response.ok(),
          url: String(response.url() || "").slice(0, 240)
        };
        if (http_mutation_observations.length < 20) {
          http_mutation_observations.push(observation);
        }
        if (!observation.ok && !http_mutation_failure) {
          http_mutation_failure = `http_mutation_failed:${method}:${observation.status}`;
        }
      } catch (_) {}
    });
    mark("server_check");
    server_check = await rawHttpGet(url);
    if (!server_check.ok) {
      const err = new Error(server_check.error || "server_unreachable");
      err.anvilFailureKind = "probe_infrastructure_failed:server_unreachable";
      throw err;
    }
    await warmUpNavigation(page, url);
    await measuredNavigation(page, url);
    mark("surface_wait");
    const surface = page.locator("canvas, button, [role=button], input, select, textarea, [contenteditable='true'], [data-anvil-action], [data-anvil-state]").first();
    await surface.waitFor({ timeout: 10000 });
    steps.push("surface_visible");
    mark("observing");
    post_js_surface = await surfaceSnapshot(page);
    surface_fit = await surfaceFitSnapshot(page);
    contract_hooks = await contractHookStatus(page);
    contract_hook_status = contract_hooks.status;
    action_hooks = contract_hooks.action_hooks || [];
    probe_mode = contract_hooks.usable ? "contract" : "heuristic";
    before_marker = await activeMarker(page, probe_mode);
    await recordCanvasSnapshot(page, "before_start");

    let initial_start_text = "";
    let transitionObserved = false;
    if (probe_mode === "contract") {
      const startControl = page.locator('[data-anvil-action="primary"]').first();
      if (await startControl.count()) {
        initial_start_text = await controlText(startControl);
        let startControlEnabled = false;
        try {
          startControlEnabled = await startControl.isEnabled();
        } catch (_) {}
        if (startControlEnabled) {
          try {
            await startControl.click({ timeout: 1200 });
            after_marker = await markerAfterActiveChange(page, probe_mode, before_marker, 800);
            if (before_marker !== after_marker) {
              primary_transition_observed = true;
              transitionObserved = true;
              steps.push("start_transition");
            } else {
              steps.push("primary_start_transition_missing");
            }
          } catch (_) {
            steps.push("primary_start_transition_missing");
          }
        } else {
          steps.push("primary_start_transition_missing");
          informational_failure_kinds.push("primary_start_control_disabled_before_text_entry");
        }
      }
      if (!transitionObserved) {
        const fallback = await attemptRankedCandidateTransitions(page, probe_mode, true);
        if (fallback.observed) {
          after_marker = fallback.after;
          transitionObserved = true;
          steps.push("start_transition");
        }
      }
    } else {
      const fallback = await attemptRankedCandidateTransitions(page, probe_mode, false);
      if (fallback.observed) {
        before_marker = fallback.before;
        after_marker = fallback.after;
        transitionObserved = true;
        primary_transition_observed = candidate_table.length > 0 && candidate_table[0].changed;
        steps.push("start_transition");
        initial_start_text = fallback.candidate ? fallback.candidate.text_excerpt : "";
      } else {
        after_marker = await settledRenderedValue(page, () => activeMarker(page, probe_mode), 500);
        primary_transition_observed = false;
      }
    }
    await recordCanvasSnapshot(page, "after_start");
    if (transitionObserved) {
      const restartReachability = await restartHookReachability(page);
      restart_hook_reachable_after_start = !!restartReachability.reachable;
      restart_hook_count_after_start = restartReachability.count || 0;
      if (restart_hook_reachable_after_start) {
        steps.push("restart_hook_reachable_after_start");
      }
    }

    const textEntryObserved = await attemptTextEntry(page, probe_mode);

    if (transitionObserved && !textEntryObserved) {
      await dispatchPostTransitionInputs(page, probe_mode);
    }

    if (!transitionObserved) {
      start_control_found = await hasStartLikeControl(page);
      if (!start_control_found && !textEntryObserved) {
        await dispatchStartlessTextInput(page, probe_mode);
      }
    }
    await page.waitForTimeout(RENDER_SETTLE_MS);
    await recordCanvasSnapshot(page, "after_inputs");

    // Persistence must observe the committed mutation before a restart/reset
    // action can change the state being compared.
    if (persistenceRequired) {
      await evaluatePersistenceReload(page, probe_mode);
    }

    if (transitionObserved && probe_mode === "contract") {
      await attemptContractRecoveryTransition(page, after_marker);
    } else {
      await attemptRecoveryTransition(page, initial_start_text, probe_mode);
    }
    if (!transitionObserved && steps.includes("recovery_transition")) {
      transitionObserved = true;
      steps.push("start_transition");
      after_marker = recovery_after_marker;
      if (!textEntryObserved) {
        await dispatchPostTransitionInputs(page, probe_mode);
      }
    }
    await recordCanvasSnapshot(page, "after_recovery");

    if (!persistenceRequired) {
      await evaluatePersistenceReload(page, probe_mode);
    }

    if (!primary_transition_observed && transitionObserved) {
      informational_failure_kinds.push("primary_start_transition_missing");
    }

    const contractInputStateChanged = textEntryRequired
      ? text_input_state_change
      : steps.includes("input_state_change") || text_input_state_change;
    const canvasNotRedrawn = !!(post_js_surface && post_js_surface.has_canvas)
      && transitionObserved
      && contractInputStateChanged
      && canvasNotRedrawnAfterStart();
    if (canvasNotRedrawn && !informational_failure_kinds.includes("canvas_not_redrawn_after_start")) {
      informational_failure_kinds.push("canvas_not_redrawn_after_start");
    }
    const inputStateChanged = contractInputStateChanged && !canvasNotRedrawn;
    if (canvasNotRedrawn) {
      for (let index = steps.length - 1; index >= 0; index -= 1) {
        if (steps[index] === "input_state_change") steps.splice(index, 1);
      }
      steps.push("input_contract_state_change");
      steps.push("canvas_not_redrawn_after_start");
    }
    const textInputObserved = text_entry === "entered" && text_input_state_change;
    const inputEvaluated = input_state_evaluated_after_start || textInputObserved;
    const startlessInputObserved = (!start_control_found && inputStateChanged) || textInputObserved;
    const canvasBlankBeforeStart = canvasBlankForSnapshot("before_start");
    const canvasBlankAfterStart = canvasBlankForSnapshot("after_start");
    const canvasBlankAfterInputs = canvasBlankForSnapshot("after_inputs");
    const canvasBlankFailure = !!(post_js_surface && post_js_surface.has_canvas)
      && transitionObserved
      && canvasBlankAfterStart === true
      && canvasBlankAfterInputs === true;
    const ok = steps.includes("surface_visible")
      && inputStateChanged
      && ((transitionObserved && inputEvaluated) || startlessInputObserved)
      && (!tokenEchoRequired || token_echoed)
      && (!persistenceRequired || persistence_after_reload === "preserved")
      && !http_mutation_failure
      && !canvasBlankFailure;
    const recoveryObserved = steps.includes("recovery_transition");
    const failureKind = http_mutation_failure
      ? http_mutation_failure
      : persistenceRequired && persistence_after_reload !== "preserved"
      ? (persistence_after_reload === "reset"
        ? "persistence_after_reload_reset"
        : `persistence_not_evaluated:${persistence_after_reload_reason || "no_committed_mutation"}`)
      : tokenEchoRequired && !token_echoed
        ? (token_echoed_after_reload ? "token_echo_after_reload_only" : "token_echo_missing")
      : textEntryRequired && text_entry !== "entered"
        ? "text_entry_missing"
      : textEntryRequired && !text_input_state_change
        ? "text_input_state_change_missing"
      : canvasBlankFailure
        ? "canvas_blank"
      : interactionFailureKind(
      transitionObserved || startlessInputObserved,
      inputEvaluated || startlessInputObserved,
      inputStateChanged
    );
    write({
      ok,
      status: ok ? "passed" : "failed",
      interaction_success: ok,
      interaction_performed: ok,
      input_event_observed: steps.includes("control_input_dispatched") || text_entry === "entered",
      input_state_change: inputStateChanged,
      input_contract_state_change: contractInputStateChanged,
      state_changed: inputStateChanged,
      visible_state_changed: inputStateChanged,
      recovery_transition: recoveryObserved,
      recovery_transition_status,
      start_transition: transitionObserved,
      start_control_found,
      primary_start_transition: primary_transition_observed,
      primary_start_transition_missing: !primary_transition_observed && transitionObserved,
      input_state_evaluated_after_start,
      probe_mode,
      contract_hook_status,
      contract_hooks,
      candidate_table,
      input_dispatches,
      canvas_snapshots,
      canvas_blank_before_start: canvasBlankBeforeStart,
      canvas_blank_after_start: canvasBlankAfterStart,
      canvas_blank_after_inputs: canvasBlankAfterInputs,
      canvas_not_redrawn_after_start: canvasNotRedrawn,
      state_dimensions_changed,
      surface_fit,
      restart_hook_reachable_after_start,
      restart_hook_count_after_start,
      persistence_after_reload,
      persistence_after_reload_reason,
      persistence_changed_dimensions,
      persistence_before_reload_marker,
      persistence_after_reload_marker,
      action_hooks,
      text_entry,
      text_entry_target,
      typed_token,
      token_echoed,
      echo_latency_ms,
      token_echoed_after_reload,
      token_echo_after_reload_latency_ms,
      text_input_state_change,
      cold_start_ms,
      measured_navigation_ms,
      warmup_attempts,
      informational_failure_kinds,
      http_mutation_observations,
      http_mutation_failure,
      steps,
      stage,
      before_marker,
      after_marker,
      input_before_marker,
      input_after_marker,
      recovery_before_marker,
      recovery_after_marker,
      failure_kind: ok ? "" : failureKind,
      server_http_status: server_check.status,
      server_check,
      post_js_has_canvas: post_js_surface ? post_js_surface.has_canvas : false,
      post_js_canvas_count: post_js_surface ? post_js_surface.canvas_count : 0,
      post_js_interactive_control_count: post_js_surface ? post_js_surface.interactive_control_count : 0,
      has_canvas: post_js_surface ? post_js_surface.has_canvas : false,
      canvas_count: post_js_surface ? post_js_surface.canvas_count : 0,
      interactive_control_count: post_js_surface ? post_js_surface.interactive_control_count : 0,
      title_text_excerpt: post_js_surface ? post_js_surface.title_text_excerpt : "",
      duration_ms: Date.now() - started
    });
    await browser.close();
    process.exit(ok ? 0 : 1);
  } catch (err) {
    if (browser) {
      try { await browser.close(); } catch (_) {}
    }
    writeFailure({
      ok: false,
      status: "failed",
      steps,
      stage,
      before_marker,
      after_marker,
      input_before_marker,
      input_after_marker,
      recovery_before_marker,
      recovery_after_marker,
      input_state_change: steps.includes("input_state_change"),
      state_changed: steps.includes("input_state_change"),
      visible_state_changed: steps.includes("input_state_change"),
      recovery_transition: steps.includes("recovery_transition"),
      recovery_transition_status,
      start_transition: steps.includes("start_transition") || steps.includes("recovery_transition"),
      start_control_found,
      primary_start_transition: primary_transition_observed,
      primary_start_transition_missing: !primary_transition_observed && (steps.includes("start_transition") || steps.includes("recovery_transition")),
      input_state_evaluated_after_start,
      probe_mode,
      contract_hook_status,
      contract_hooks,
      candidate_table,
      input_dispatches,
      canvas_snapshots,
      canvas_blank_before_start: canvasBlankForSnapshot("before_start"),
      canvas_blank_after_start: canvasBlankForSnapshot("after_start"),
      canvas_blank_after_inputs: canvasBlankForSnapshot("after_inputs"),
      canvas_not_redrawn_after_start: canvasNotRedrawnAfterStart(),
      state_dimensions_changed,
      surface_fit,
      restart_hook_reachable_after_start,
      restart_hook_count_after_start,
      persistence_after_reload,
      persistence_after_reload_reason,
      persistence_changed_dimensions,
      persistence_before_reload_marker,
      persistence_after_reload_marker,
      action_hooks,
      text_entry,
      text_entry_target,
      typed_token,
      token_echoed,
      echo_latency_ms,
      token_echoed_after_reload,
      token_echo_after_reload_latency_ms,
      text_input_state_change,
      cold_start_ms,
      measured_navigation_ms,
      warmup_attempts,
      informational_failure_kinds,
      http_mutation_observations,
      http_mutation_failure,
      failure_kind: err && err.anvilFailureKind ? err.anvilFailureKind : "probe_script_error",
      navigation_failure_kind: err && err.navigationFailureKind ? err.navigationFailureKind : "",
      error: err && err.message ? err.message : String(err),
      server_http_status: server_check.status,
      server_http_error: server_check.error || "",
      server_check,
      post_js_has_canvas: post_js_surface ? post_js_surface.has_canvas : null,
      post_js_canvas_count: post_js_surface ? post_js_surface.canvas_count : null,
      post_js_interactive_control_count: post_js_surface ? post_js_surface.interactive_control_count : null,
      has_canvas: post_js_surface ? post_js_surface.has_canvas : null,
      canvas_count: post_js_surface ? post_js_surface.canvas_count : null,
      interactive_control_count: post_js_surface ? post_js_surface.interactive_control_count : null,
      title_text_excerpt: post_js_surface ? post_js_surface.title_text_excerpt : "",
      duration_ms: Date.now() - started
    });
    process.exit(1);
  }
})();
