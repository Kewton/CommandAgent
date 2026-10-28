// Persistence commit witness (IC-44).
//
// An editor that saves an existing record shows the typed text in the same state
// key before and after the save, so the contract probe finds no commit key and
// leaves persistence not evaluated. This leaf gathers a stricter witness for that
// case only, and every part of it is required:
//   1. the probe's fresh token was not on the page or in a read before text entry;
//   2. a same-origin mutation that started inside the save press the probe made
//      after text entry carried the token and completed with 2xx before the reload;
//   3. after the existing reload no text entry is displayed and the token is
//      visible in a text node outside any text entry;
//   4. a read started after the reload returned the saved resource with the token.
// Anything less leaves persistence not evaluated. Request and response bodies are
// read in memory only; the probe keeps booleans and the recorded steps.

const PERSISTENCE_COMMIT_MUTATION_METHODS = ["POST", "PUT", "PATCH"];
const PERSISTENCE_COMMIT_SAVE_GRACE_MS = 250;
const PERSISTENCE_COMMIT_SETTLE_MS = 3000;
const PERSISTENCE_COMMIT_SURFACE_MS = 3000;
const PERSISTENCE_COMMIT_POLL_MS = 100;
const PERSISTENCE_COMMIT_BODY_LIMIT = 262144;
const PERSISTENCE_COMMIT_MAX_REQUESTS = 400;
const PERSISTENCE_COMMIT_MAX_JSON_NODES = 20000;

function persistenceCommitUrlParts(text) {
  try {
    const parsed = new URL(String(text || ""));
    return { origin: parsed.origin, path: parsed.pathname.replace(/\/+$/, "") || "/" };
  } catch (_) {
    return { origin: "", path: "" };
  }
}

// True when some object in `value` names `id` in one of its own scalar fields and
// carries `token` somewhere inside it: the record of that id still holds the token.
function persistenceCommitRecordHoldsToken(value, id, token) {
  let visited = 0;
  const holdsToken = (node) => {
    try {
      return JSON.stringify(node).includes(token);
    } catch (_) {
      return false;
    }
  };
  const walk = (node, depth) => {
    visited += 1;
    if (visited > PERSISTENCE_COMMIT_MAX_JSON_NODES || depth > 8 || !node || typeof node !== "object") {
      return false;
    }
    const children = Array.isArray(node) ? node : Object.values(node);
    if (!Array.isArray(node)) {
      const named = Object.values(node).some((field) =>
        (typeof field === "string" || typeof field === "number") && String(field) === id
      );
      if (named && holdsToken(node)) return true;
    }
    return children.some((child) => walk(child, depth + 1));
  };
  return walk(value, 0);
}

function createPersistenceCommitWitness(probeUrl, token) {
  const origin = persistenceCommitUrlParts(probeUrl).origin;
  const records = new Map();
  const bodyReads = [];
  let phase = "before_entry";
  let entryCheckClean = false;
  let saveStartedAt = 0;
  let saveWindowEndsAt = 0;
  let reloadMarked = false;
  let tokenSeenBeforeEntry = false;

  const currentPhase = () => {
    if (phase === "save" && saveWindowEndsAt && Date.now() > saveWindowEndsAt) {
      phase = "after_save";
    }
    return phase;
  };

  const isMutation = (record) => PERSISTENCE_COMMIT_MUTATION_METHODS.includes(record.method);

  // A save is a same-origin mutation that started inside the save press and carried
  // the token in its payload.
  const saveRecords = () => Array.from(records.values()).filter((record) =>
    record.phase === "save" && record.same_origin && isMutation(record) && record.payload_has_token
  );

  const onRequest = (request) => {
    try {
      if (records.size >= PERSISTENCE_COMMIT_MAX_REQUESTS) return;
      const method = String(request.method() || "").toUpperCase();
      // Documents, scripts, styles and media are neither a save nor a read of a record.
      const resourceType = typeof request.resourceType === "function" ? String(request.resourceType() || "") : "";
      if (!PERSISTENCE_COMMIT_MUTATION_METHODS.includes(method) && !["fetch", "xhr"].includes(resourceType)) {
        return;
      }
      const requestUrl = String(request.url() || "");
      const parts = persistenceCommitUrlParts(requestUrl);
      const record = {
        method,
        path: parts.path,
        same_origin: !!origin && parts.origin === origin,
        url_has_token: !!token && requestUrl.includes(token),
        phase: currentPhase(),
        payload_has_token: false,
        status: null,
        ok: false,
        failed: false,
        settled_before_reload: false,
        body_has_token: false,
        body_json: undefined
      };
      if (PERSISTENCE_COMMIT_MUTATION_METHODS.includes(method)) {
        let payload = "";
        try {
          payload = String(request.postData() || "");
        } catch (_) {}
        record.payload_has_token = !!token && payload.includes(token);
      }
      records.set(request, record);
    } catch (_) {}
  };

  const onResponse = (response) => {
    try {
      const record = records.get(response.request());
      if (!record) return;
      record.status = response.status();
      record.ok = record.status >= 200 && record.status < 300;
      record.settled_before_reload = !reloadMarked;
      // A request the browser started before the save press is never the save, even
      // when its event or response arrives inside the press.
      if (record.phase === "save" && saveStartedAt && typeof response.request().timing === "function") {
        const startTime = Number((response.request().timing() || {}).startTime || 0);
        if (startTime > 0 && startTime < saveStartedAt) record.phase = "entry";
      }
      // Only a read answered before text entry can show that the token was already
      // stored; one answered later may hold what the probe typed.
      const answeredBeforeEntry = record.phase === "before_entry" && currentPhase() === "before_entry";
      if (
        isMutation(record) ||
        !record.same_origin ||
        !record.ok ||
        !(answeredBeforeEntry || record.phase === "after_reload")
      ) {
        return;
      }
      bodyReads.push(Promise.resolve()
        .then(() => response.text())
        .then((text) => {
          const body = String(text || "");
          if (body.length > PERSISTENCE_COMMIT_BODY_LIMIT) return;
          record.body_has_token = !!token && body.includes(token);
          if (answeredBeforeEntry) {
            if (record.body_has_token) tokenSeenBeforeEntry = true;
            return;
          }
          try {
            record.body_json = JSON.parse(body);
          } catch (_) {}
        })
        .catch(() => {}));
    } catch (_) {}
  };

  const onRequestFailed = (request) => {
    try {
      const record = records.get(request);
      if (record) record.failed = true;
    } catch (_) {}
  };

  const settledBodyReads = async () => {
    let timer;
    const deadline = new Promise((resolve) => {
      timer = setTimeout(resolve, PERSISTENCE_COMMIT_SETTLE_MS);
    });
    await Promise.race([Promise.all(bodyReads.slice()), deadline]);
    clearTimeout(timer);
  };

  // The read that names the saved resource: the same path, or its parent collection
  // holding a record whose id is the saved path's last segment.
  const readback = (saves) => {
    const result = { matched: false, exact_without_token: false };
    const reads = Array.from(records.values()).filter((record) =>
      record.phase === "after_reload" &&
      record.same_origin &&
      !isMutation(record) &&
      record.ok &&
      !record.failed &&
      !record.url_has_token
    );
    for (const read of reads) {
      for (const save of saves) {
        if (read.path === save.path) {
          if (read.body_has_token) result.matched = true;
          else if (read.body_json !== undefined) result.exact_without_token = true;
          continue;
        }
        if (!read.path || !save.path.startsWith(`${read.path}/`)) continue;
        const rest = save.path.slice(read.path.length + 1);
        if (!rest || rest.includes("/")) continue;
        let id = rest;
        try {
          id = decodeURIComponent(rest);
        } catch (_) {}
        if (read.body_has_token && persistenceCommitRecordHoldsToken(read.body_json, id, token)) {
          result.matched = true;
        }
      }
    }
    return result;
  };

  const surfaceNow = async (page) => await page.evaluate((value) => {
    const entrySelector = 'textarea, input:not([type]), input[type=""], input[type="text"], input[type="search"], [contenteditable=""], [contenteditable="true"], [contenteditable="plaintext-only"]';
    const isEntry = (element) => {
      const tag = (element.tagName || "").toLowerCase();
      if (tag === "input" || tag === "textarea") return true;
      const attr = element.getAttribute ? element.getAttribute("contenteditable") : null;
      const editable = attr === null ? null : String(attr).toLowerCase();
      return editable === "" || editable === "true" || editable === "plaintext-only";
    };
    const shown = (element) => {
      for (let current = element; current; current = current.parentElement) {
        const style = window.getComputedStyle(current);
        if (style.display === "none" || style.visibility === "hidden" || Number(style.opacity || "1") <= 0) {
          return false;
        }
        if (current.getAttribute && (
          current.getAttribute("aria-hidden") === "true" ||
          current.getAttribute("hidden") !== null
        )) {
          return false;
        }
      }
      const box = element.getBoundingClientRect();
      return box.width > 0 && box.height > 0;
    };
    const editorPresent = Array.from(document.querySelectorAll(entrySelector)).some((element) => shown(element));
    let tokenVisible = false;
    const walker = document.createTreeWalker(document.body || document.documentElement, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node && !tokenVisible; node = walker.nextNode()) {
      if (!(node.nodeValue || "").includes(value)) continue;
      const parent = node.parentElement;
      if (!parent) continue;
      let insideEntry = false;
      for (let current = parent; current; current = current.parentElement) {
        if (isEntry(current)) insideEntry = true;
      }
      if (!insideEntry && shown(parent)) tokenVisible = true;
    }
    return { editor_present: editorPresent, token_visible: tokenVisible };
  }, token);

  return {
    attach(page) {
      page.on("request", onRequest);
      page.on("response", onResponse);
      page.on("requestfailed", onRequestFailed);
    },

    // Called before the probe types: the token must not be on the page yet.
    async markEntry(page) {
      phase = "entry";
      try {
        const present = await page.evaluate((value) => {
          const root = document.body || document.documentElement;
          if (root && String(root.textContent || "").includes(value)) return true;
          return Array.from(document.querySelectorAll("input, textarea"))
            .some((element) => String(element.value || "").includes(value));
        }, token);
        const urlHasToken = String(page.url() || "").includes(token);
        entryCheckClean = !!token && present === false && !urlHasToken;
      } catch (_) {
        entryCheckClean = false;
      }
    },

    markSaveStart() {
      if (phase !== "entry") return;
      phase = "save";
      saveStartedAt = Date.now();
      saveWindowEndsAt = 0;
    },

    markSaveEnd() {
      if (phase === "save") saveWindowEndsAt = Date.now() + PERSISTENCE_COMMIT_SAVE_GRACE_MS;
    },

    // Waits, bounded, for the saves the press started to finish before the reload.
    async awaitSaveSettled(page) {
      const deadline = Date.now() + PERSISTENCE_COMMIT_SETTLE_MS;
      while (Date.now() < deadline) {
        const pending = currentPhase() === "save" ||
          saveRecords().some((record) => record.status === null && !record.failed);
        if (!pending) return;
        await page.waitForTimeout(PERSISTENCE_COMMIT_POLL_MS);
      }
    },

    markReload() {
      reloadMarked = true;
      phase = "after_reload";
    },

    // Returns the verdict ("preserved", "reset" or "" for not evaluated) and the steps
    // that record which parts of the witness were observed.
    async evaluate(page, context) {
      const result = { verdict: "", steps: [] };
      const eligible = context.mode === "contract" &&
        context.legacy_keys.length === 0 &&
        context.text_entry === "entered" &&
        context.text_input_state_change &&
        !context.http_mutation_failure;
      if (!eligible || !token || !entryCheckClean || !reloadMarked) return result;
      await settledBodyReads();
      if (tokenSeenBeforeEntry) return result;
      const saves = saveRecords();
      const saved = saves.length > 0 && saves.every((record) =>
        record.ok && !record.failed && record.settled_before_reload
      );
      if (!saved) return result;
      result.steps.push("persistence_commit_http_observed");
      let surface = { editor_present: true, token_visible: false };
      try {
        const deadline = Date.now() + PERSISTENCE_COMMIT_SURFACE_MS;
        while (true) {
          surface = await surfaceNow(page);
          if ((surface.token_visible && !surface.editor_present) || Date.now() >= deadline) break;
          await page.waitForTimeout(PERSISTENCE_COMMIT_POLL_MS);
        }
        if (String(page.url() || "").includes(token)) return result;
      } catch (_) {
        return result;
      }
      if (surface.editor_present) return result;
      await settledBodyReads();
      const read = readback(saves);
      if (surface.token_visible) result.steps.push("persistence_committed_surface_observed");
      if (read.matched) result.steps.push("persistence_committed_readback_observed");
      if (surface.token_visible && read.matched) {
        result.verdict = "preserved";
      } else if (!surface.token_visible && !read.matched && read.exact_without_token) {
        result.verdict = "reset";
      }
      return result;
    }
  };
}
