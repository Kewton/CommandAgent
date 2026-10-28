// A fake Playwright module for the persistence commit witness (IC-44). The real
// probe script drives it. It models a notes app that edits an existing record:
// a list, a start press that creates a record, an editor with a live preview, a
// save press, and a reload. Every request, response and request failure is
// emitted through the page's own listeners at the moment the app would send or
// receive it, and every effect follows the probe's own presses, keystrokes and
// reloads. The server store outlives a reload; the page state does not.
//
// The scenario name is substituted for __SCENARIO__ when the module is installed.
// Each scenario is the positive app with one or more parts changed; see
// scenarios.json beside this file.

const scenario = "__SCENARIO__";
const ELEMENT_NODE = 1;
const TEXT_NODE = 3;

const RECORDED = {
  keys: { content: "content", dirty: "dirty", mode: "mode", count: "noteCount", selected: "selectedId", title: "title" },
  modes: { list: "list", edit: "edit" },
  collection: "/api/notes",
  seed: [
    { id: "seed-1", title: "買い物", content: "牛乳を買う" },
    { id: "seed-2", title: "会議", content: "議題を確認する" }
  ],
  created: { id: "c5d50490-c008-4c37-9313-494ff91c096f", title: "無題のノート", content: "# 新しいノート\n" },
  prependTyping: true
};

const RENAMED = {
  keys: { content: "text", dirty: "unsaved", mode: "screen", count: "total", selected: "openId", title: "name" },
  modes: { list: "index", edit: "compose" },
  collection: "/data/memos",
  seed: [{ id: "memo-1", title: "Groceries", content: "eggs" }],
  created: { id: "memo-7f3a", title: "Untitled memo", content: "Draft\n" },
  prependTyping: false
};

const DEFAULTS = {
  ...RECORDED,
  // "content_g": the L-135 editor route that stays on the same editor after a reload.
  shape: "list_editor",
  save: "put",
  savePath: "record",
  saveOrigin: "same",
  saveStatus: 200,
  saveFails: false,
  saveNeverResponds: false,
  serverStores: true,
  autosave: false,
  holdAutosaveResponse: false,
  saveClearsDirtyOnly: false,
  localMirror: false,
  afterReload: "list",
  listShowsContent: true,
  listHidden: false,
  readsRecordAfterReload: false,
  listKey: "notes",
  listOmitsContent: false,
  listPageField: false,
  storeAsId: "",
  saveDisabled: false,
  autosaveOnBlockedPress: false,
  deferAutosaveEvent: false,
  tokenInEarlyRead: false,
  crossOriginRead: false,
  saveAnswersAfterReload: false,
  staleToken: "",
  tokenBeforeEntry: false,
  urlEcho: false,
  reloadThrows: false
};

const SCENARIOS = {
  recorded_content_q: {},
  synthetic_renamed: { ...RENAMED },
  l135_editor_after_reload: {
    shape: "content_g",
    keys: { content: "content", title: "title" },
    created: { id: "new", title: "", content: "" },
    prependTyping: false
  },
  save_without_change: { save: "put_unchanged", localMirror: true },
  dirty_only: { save: "none", saveClearsDirtyOnly: true, localMirror: true },
  count_only: { save: "none" },
  stale_token: { serverStores: false, staleToken: "anvil-zz9old" },
  create_post_only: { save: "none", localMirror: true },
  autosave_only: { save: "none", autosave: true },
  late_response: { save: "none", autosave: true, holdAutosaveResponse: true },
  cross_origin: { saveOrigin: "other" },
  other_resource: { savePath: "other_resource" },
  readback_without_token: { serverStores: false, localMirror: true },
  list_without_token: { listShowsContent: false },
  entry_only_after_reload: { afterReload: "entry_only" },
  preview_only: { serverStores: false, localMirror: true, afterReload: "preview_only" },
  hidden_only: { listHidden: true },
  local_storage_draft: { save: "none", localMirror: true },
  http_4xx: { saveStatus: 409 },
  http_5xx: { saveStatus: 500 },
  request_failed: { saveFails: true },
  reload_failed: { reloadThrows: true },
  save_timeout: { saveNeverResponds: true },
  token_before_entry: { tokenBeforeEntry: true },
  url_echo: { urlEcho: true },
  readback_exact_without_token: { serverStores: false, readsRecordAfterReload: true, listShowsContent: false },
  collection_post_titles_only: {
    save: "post_collection",
    collection: "/api/memos",
    listKey: "memos",
    created: { id: "m1", title: "メモ", content: "" },
    listOmitsContent: true,
    listShowsContent: false
  },
  disabled_press_debounced_autosave: { save: "none", saveDisabled: true, autosaveOnBlockedPress: true },
  id_field_collision: {
    seed: [...RECORDED.seed, { id: "5", title: "別のノート", content: "別の本文" }],
    created: { id: "1", title: "無題のノート", content: "# 新しいノート\n" },
    storeAsId: "5",
    listPageField: true
  },
  late_event_early_start: { save: "none", autosave: true, deferAutosaveEvent: true },
  token_in_read_before_entry: { tokenInEarlyRead: true },
  cross_origin_readback: { serverStores: false, localMirror: true, crossOriginRead: true },
  save_answers_after_reload: { saveAnswersAfterReload: true },
  id_like_field_in_element: {
    seed: [...RECORDED.seed, { id: "9", order: 1, title: "別のノート", content: "別の本文" }],
    created: { id: "1", title: "無題のノート", content: "# 新しいノート\n" },
    storeAsId: "9"
  }
};

if (!Object.prototype.hasOwnProperty.call(SCENARIOS, scenario)) {
  throw new Error(`unknown fake scenario ${scenario}`);
}
const cfg = { ...DEFAULTS, ...SCENARIOS[scenario] };

// What outlives a reload: the server store and the browser's local storage.
const server = { notes: cfg.seed.map((note) => ({ ...note })) };
if (cfg.staleToken) server.notes.push({ id: "seed-stale", title: "古いノート", content: cfg.staleToken });
const local = { notes: null };

class FakeElement {
  constructor(page, tag, attrs = {}, text = "") {
    this.page = page;
    this.tagName = tag.toUpperCase();
    this.attrs = { ...attrs };
    this.style = {};
    this.disabled = false;
    this.value = "";
    this.text = text;
    this.parentElement = null;
    this.nodeType = ELEMENT_NODE;
    this.box = { left: 0, top: 0, width: 200, height: 24 };
  }

  get textContent() {
    return this.page.textNodes()
      .filter((node) => this.contains(node.parentElement))
      .map((node) => node.nodeValue)
      .join(" ");
  }

  get innerText() {
    return this.textContent;
  }

  getAttribute(name) {
    if (name === "data-anvil-state" && this === this.page.main) {
      return JSON.stringify(this.page.snapshot());
    }
    return Object.prototype.hasOwnProperty.call(this.attrs, name) ? this.attrs[name] : null;
  }

  setAttribute(name, value) {
    this.attrs[name] = String(value);
  }

  contains(node) {
    for (let current = node; current; current = current.parentElement) {
      if (current === this) return true;
    }
    return false;
  }

  getBoundingClientRect() {
    const { left, top, width, height } = this.box;
    return { left, top, width, height, right: left + width, bottom: top + height, x: left, y: top };
  }
}

class FakeTextNode {
  constructor(value, parentElement) {
    this.nodeType = TEXT_NODE;
    this.nodeValue = value;
    this.parentElement = parentElement;
  }
}

class FakeDocument {
  constructor(page) {
    this.page = page;
    this.title = "Fake notes";
    this.body = page.body;
    this.documentElement = page.root;
  }

  get activeElement() {
    return this.page.activeElement || this.page.body;
  }

  querySelectorAll(selector) {
    return this.page.querySelectorAll(selector);
  }

  querySelector(selector) {
    return this.querySelectorAll(selector)[0] || null;
  }

  createTreeWalker() {
    const nodes = this.page.textNodes();
    let index = 0;
    return {
      nextNode() {
        return nodes[index++] || null;
      }
    };
  }
}

class FakeLocator {
  constructor(page, selector, elements) {
    this.page = page;
    this.selector = selector;
    this.elements = elements;
  }

  first() {
    return new FakeLocator(this.page, this.selector, this.elements.slice(0, 1));
  }

  nth(index) {
    return new FakeLocator(this.page, this.selector, this.elements[index] ? [this.elements[index]] : []);
  }

  async count() {
    return this.elements.length;
  }

  async isEnabled() {
    return this.elements.length > 0 && !this.elements[0].disabled;
  }

  async waitFor() {
    if (!this.elements.length) throw new Error(`no element for ${this.selector}`);
  }

  async evaluateAll(fn, arg) {
    this.page.installGlobals();
    return fn(this.elements, arg);
  }

  async evaluate(fn, arg) {
    if (!this.elements.length) throw new Error(`no element for ${this.selector}`);
    this.page.installGlobals();
    return fn(this.elements[0], arg);
  }

  async click() {
    if (!this.elements.length) throw new Error(`no element for ${this.selector}`);
    if (this.elements[0].disabled) {
      // A debounced autosave fires while the probe is trying to press.
      this.page.blockedPress();
      throw new Error(`element is not enabled for ${this.selector}`);
    }
    this.page.clickElement(this.elements[0]);
  }

  async focus() {
    if (!this.elements.length) throw new Error(`no element for ${this.selector}`);
    this.page.activeElement = this.elements[0];
  }

  async boundingBox() {
    if (!this.elements.length) return null;
    const box = this.elements[0].getBoundingClientRect();
    return { x: box.left, y: box.top, width: box.width, height: box.height };
  }
}

class FakeRequest {
  constructor(method, url, payload) {
    this._method = method;
    this._url = url;
    this._payload = payload;
    this._startTime = Date.now();
  }

  method() {
    return this._method;
  }

  url() {
    return this._url;
  }

  postData() {
    return this._payload;
  }

  resourceType() {
    return "fetch";
  }

  timing() {
    return { startTime: this._startTime };
  }
}

class FakeResponse {
  constructor(request, status, body) {
    this._request = request;
    this._status = status;
    this._body = body;
  }

  request() {
    return this._request;
  }

  status() {
    return this._status;
  }

  ok() {
    return this._status >= 200 && this._status < 300;
  }

  url() {
    return this._request.url();
  }

  async text() {
    return this._text ? await this._text(this._body) : this._body;
  }
}

class FakePage {
  constructor() {
    this.listeners = { request: [], response: [], requestfailed: [] };
    this.origin = "";
    this.held = [];
    this.heldUntilReload = [];
    this.learnedToken = new Promise((resolve) => {
      this.learnToken = resolve;
    });
    this.typed = false;
    this.urlSuffix = "";
    this.route = "list";
    this.lastEditedId = "";
    this.preseededToken = "";
    this.root = new FakeElement(this, "html");
    this.body = new FakeElement(this, "body");
    this.body.parentElement = this.root;
    this.main = new FakeElement(this, "main", { "data-anvil-state": "" });
    this.main.parentElement = this.body;
    this.main.box = { left: 0, top: 0, width: 1024, height: 720 };
    this.primary = new FakeElement(this, "button", { "data-anvil-action": "primary" });
    this.primary.parentElement = this.main;
    this.primary.box = { left: 500, top: 20, width: 140, height: 40 };
    this.newEntry();
    this.activeElement = null;
    this.keyboard = {
      type: async (text, options = {}) => {
        if (this.activeElement === this.entry) this.typeInto(text);
        // Typing takes the per-key delay the probe asks for.
        await this.waitForTimeout((options.delay || 0) * text.length);
      },
      down: async () => {},
      up: async () => {},
      press: async () => {}
    };
    this.mouse = { click: async () => {} };
    this.resetClient();
  }

  newEntry() {
    const editable = cfg.afterReload === "entry_only" && this.typed;
    this.entry = editable
      ? new FakeElement(this, "div", { contenteditable: "true", "data-anvil-action": "input" })
      : new FakeElement(this, "textarea", { "data-anvil-action": "input" });
    this.entry.parentElement = this.main;
    this.entry.box = { left: 40, top: 120, width: 460, height: 320 };
  }

  resetClient() {
    this.loading = true;
    this.notes = [];
    this.editingId = null;
    this.title = "";
    this.content = "";
    this.savedLocally = false;
    this.status = "";
  }

  // ----- network -----

  on(name, handler) {
    if (this.listeners[name]) this.listeners[name].push(handler);
  }

  emit(name, value) {
    for (const handler of this.listeners[name] || []) handler(value);
  }

  fetch(method, path, payload, options = {}) {
    const url = `${options.origin || this.origin}${path}`;
    const request = new FakeRequest(method, url, payload);
    if (options.deferEvent) {
      // The request started now, but its event reaches the page only at the next press.
      this.held.push(() => {
        this.fetchStarted(request, options);
      });
      return;
    }
    this.fetchStarted(request, options);
  }

  fetchStarted(request, options) {
    this.emit("request", request);
    const finish = () => {
      if (options.fail) {
        this.emit("requestfailed", request);
        return;
      }
      const status = options.status || 200;
      const response = new FakeResponse(request, status, options.body ? options.body() : "{}");
      if (options.text) response._text = options.text;
      this.emit("response", response);
      if (options.after) options.after(response, status);
    };
    if (options.never) return;
    if (options.hold) {
      this.held.push(finish);
      return;
    }
    if (options.holdUntilReload) {
      this.heldUntilReload.push(finish);
      return;
    }
    setTimeout(finish, 0);
  }

  recordPath(id) {
    if (cfg.savePath === "other_resource") return `/api/drafts/${encodeURIComponent(id)}`;
    return `${cfg.collection}/${encodeURIComponent(id)}`;
  }

  serverNote(id) {
    return server.notes.find((note) => note.id === id);
  }

  store(id, title, content) {
    const note = this.serverNote(id);
    if (note) {
      note.title = title;
      note.content = content;
    } else {
      server.notes.push({ id, title, content });
    }
  }

  refresh(then) {
    this.fetch("GET", cfg.collection, null, {
      body: () => {
        const list = cfg.listOmitsContent
          ? server.notes.map((note) => ({ id: note.id, title: note.title }))
          : server.notes;
        return JSON.stringify(cfg.listPageField ? { page: 1, [cfg.listKey]: list } : { [cfg.listKey]: list });
      },
      // A list answered before typing already holds the probe's token in a record
      // the page does not render. The fake learns the token only when the probe
      // first looks for it, so the body is completed when the probe reads it.
      text: cfg.tokenInEarlyRead && !this.typed
        ? async (body) => {
          const token = await this.learnedToken;
          const parsed = JSON.parse(body);
          parsed[cfg.listKey].push({ id: "seed-early", title: "以前のノート", content: token });
          return JSON.stringify(parsed);
        }
        : null,
      after: (response) => {
        this.notes = JSON.parse(response._body)[cfg.listKey];
        this.loading = false;
        if (then) then();
      }
    });
  }

  load() {
    this.resetClient();
    if (cfg.shape === "content_g" && this.route === "editor") {
      const id = this.lastEditedId;
      this.editingId = id;
      this.fetch("GET", `${cfg.collection}/${encodeURIComponent(id)}`, null, {
        body: () => JSON.stringify({ note: this.serverNote(id) || null }),
        after: (response) => {
          const note = JSON.parse(response._body).note;
          if (note) {
            this.title = note.title;
            this.content = note.content;
          }
          this.loading = false;
        }
      });
      return;
    }
    this.route = "list";
    this.refresh();
    if (cfg.crossOriginRead && local.notes) {
      // Another origin serves the mirrored list that the page renders.
      this.fetch("GET", cfg.collection, null, {
        origin: this.origin.replace("127.0.0.1", "127.0.0.2"),
        body: () => JSON.stringify({ [cfg.listKey]: local.notes })
      });
    }
    if (cfg.readsRecordAfterReload && this.lastEditedId) {
      const id = this.lastEditedId;
      this.fetch("GET", `${cfg.collection}/${encodeURIComponent(id)}`, null, {
        body: () => JSON.stringify({ note: this.serverNote(id) || null })
      });
    }
  }

  // ----- the app -----

  clickElement(el) {
    if (el !== this.primary) return;
    if (this.route === "list") this.start();
    else this.save();
  }

  start() {
    if (cfg.shape === "content_g") {
      this.route = "editor";
      this.editingId = cfg.created.id;
      this.lastEditedId = cfg.created.id;
      this.title = cfg.created.title;
      this.content = cfg.created.content;
      this.loading = false;
      return;
    }
    const created = { ...cfg.created };
    this.fetch("POST", cfg.collection, JSON.stringify({ title: created.title, content: created.content }), {
      status: 201,
      body: () => JSON.stringify({ note: created }),
      after: () => {
        this.store(created.id, created.title, created.content);
        this.refresh(() => {
          this.route = "editor";
          this.editingId = created.id;
          this.lastEditedId = created.id;
          this.title = created.title;
          this.content = created.content;
        });
      }
    });
  }

  typeInto(text) {
    this.typed = true;
    this.content = cfg.prependTyping ? `${text}${this.content}` : `${this.content}${text}`;
    this.savedLocally = false;
    if (cfg.localMirror) this.mirrorLocally();
    if (cfg.autosave) this.autosave();
  }

  autosave() {
    const id = this.editingId;
    const content = this.content;
    this.store(id, this.title, content);
    this.fetch("PUT", this.recordPath(id), JSON.stringify({ title: this.title, content }), {
      hold: cfg.holdAutosaveResponse,
      deferEvent: cfg.deferAutosaveEvent,
      body: () => JSON.stringify({ note: this.serverNote(id) }),
      after: () => this.refresh()
    });
  }

  blockedPress() {
    if (!cfg.autosaveOnBlockedPress || this.autosaved || !this.typed) return;
    this.autosaved = true;
    this.autosave();
  }

  mirrorLocally() {
    const notes = (local.notes || server.notes).map((note) => ({ ...note }));
    const index = notes.findIndex((note) => note.id === this.editingId);
    const edited = { id: this.editingId, title: this.title, content: this.content };
    if (index >= 0) notes[index] = edited;
    else notes.push(edited);
    local.notes = notes;
  }

  save() {
    // A response the app was still waiting for arrives during the press.
    const held = this.held.splice(0);
    for (const finish of held) finish();
    if (cfg.urlEcho) this.urlSuffix = `?draft=${encodeURIComponent(this.content)}`;
    if (cfg.save === "none") {
      if (cfg.saveClearsDirtyOnly) this.savedLocally = true;
      return;
    }
    const id = this.editingId;
    const original = this.serverNote(id);
    const content = cfg.save === "put_unchanged" && original ? original.content : this.content;
    if (cfg.serverStores && cfg.save !== "put_unchanged") this.store(cfg.storeAsId || id, this.title, content);
    if (cfg.save === "put_unchanged") this.store(id, this.title, content);
    const origin = cfg.saveOrigin === "other" ? this.origin.replace("127.0.0.1", "127.0.0.2") : this.origin;
    const toCollection = cfg.save === "post_collection";
    this.fetch(toCollection ? "POST" : "PUT", toCollection ? cfg.collection : this.recordPath(id), JSON.stringify({ id, title: this.title, content }), {
      origin,
      status: toCollection ? 201 : cfg.saveStatus,
      fail: cfg.saveFails,
      never: cfg.saveNeverResponds,
      holdUntilReload: cfg.saveAnswersAfterReload,
      body: () => JSON.stringify({ note: this.serverNote(id) || null }),
      after: (_response, status) => {
        if (status >= 200 && status < 300) this.refresh(() => {
          this.status = "保存しました。";
        });
        else this.status = "保存に失敗しました。";
      }
    });
  }

  snapshot() {
    const k = cfg.keys;
    if (cfg.shape === "content_g") {
      if (this.route !== "editor") return { notes: this.notes.map((note) => note.id) };
      return { [k.content]: this.content, [k.title]: this.title };
    }
    const editing = this.route === "editor";
    const original = this.notes.find((note) => note.id === this.editingId);
    const dirty = editing && !this.savedLocally &&
      (!original || original.title !== this.title || original.content !== this.content);
    return {
      [k.content]: this.content,
      [k.dirty]: dirty,
      [k.mode]: editing ? cfg.modes.edit : cfg.modes.list,
      [k.count]: this.notes.length,
      [k.selected]: this.editingId,
      [k.title]: this.title
    };
  }

  // ----- rendering -----

  view() {
    const out = [];
    const editing = this.route === "editor";
    const reloaded = this.typed && this.reloadCount > 0;
    this.primary.text = editing ? "Save" : "Create New Note";
    this.primary.disabled = editing && cfg.saveDisabled;
    out.push(this.primary);
    if (editing) {
      this.entry.value = this.content;
      out.push(this.entry);
      out.push(this.element("div", `Preview ${this.content}`, { left: 520, top: 120, width: 460, height: 320 }));
    } else if (reloaded && cfg.afterReload === "entry_only") {
      this.entry.text = this.lastContent();
      out.push(this.entry);
    } else if (reloaded && cfg.afterReload === "preview_only") {
      out.push(this.element("div", `Preview ${this.lastContent()}`, { left: 40, top: 120, width: 460, height: 320 }));
    } else if (!this.loading) {
      const list = this.element("ul", "", { left: 40, top: 120, width: 900, height: 400 });
      if (cfg.listHidden) list.style.display = "none";
      out.push(list);
      const notes = local.notes || this.notes;
      notes.forEach((note, index) => {
        const label = cfg.listShowsContent ? `${note.title} ${note.content.slice(0, 120)}` : note.title;
        const item = this.element("li", label, { left: 40, top: 120 + index * 48, width: 900, height: 40 });
        item.parentElement = list;
        out.push(item);
      });
    }
    if (this.status) out.push(this.element("p", this.status, { left: 40, top: 80, width: 300, height: 24 }));
    if (this.preseededToken) out.push(this.element("p", `最近 ${this.preseededToken}`, { left: 40, top: 60, width: 300, height: 24 }));
    return out;
  }

  lastContent() {
    const note = this.serverNote(this.lastEditedId);
    const mirrored = (local.notes || []).find((entry) => entry.id === this.lastEditedId);
    return (mirrored || note || { content: "" }).content;
  }

  element(tag, text, box) {
    const el = new FakeElement(this, tag, {}, text);
    el.parentElement = this.main;
    el.box = box;
    return el;
  }

  elements() {
    return [this.root, this.body, this.main, ...this.view()];
  }

  textNodes() {
    const nodes = [new FakeTextNode("Markdown ノート", this.main)];
    for (const el of this.view()) {
      const text = el === this.entry ? (el.attrs.contenteditable ? el.text : "") : el.text;
      if (text) nodes.push(new FakeTextNode(text, el));
    }
    return nodes;
  }

  installGlobals() {
    global.window = {
      innerWidth: 1280,
      innerHeight: 720,
      getComputedStyle: (el) => ({
        visibility: "visible",
        display: "block",
        opacity: "1",
        pointerEvents: "auto",
        ...((el && el.style) || {})
      })
    };
    global.document = new FakeDocument(this);
    global.Node = { ELEMENT_NODE, TEXT_NODE };
    global.NodeFilter = { SHOW_TEXT: 4 };
  }

  querySelectorAll(selector) {
    const all = this.elements();
    const out = [];
    const add = (el) => {
      if (el && !out.includes(el)) out.push(el);
    };
    const isTextarea = (el) => el.tagName === "TEXTAREA";
    const isEditable = (el) => el.attrs.contenteditable === "true";
    for (const raw of String(selector || "").split(",")) {
      const part = raw.trim();
      if (!part) continue;
      if (part === "*") {
        all.forEach(add);
      } else if (part.startsWith("[data-anvil-probe-text-target")) {
        all.filter((el) => el.attrs["data-anvil-probe-text-target"] === "1").forEach(add);
      } else if (part === '[data-anvil-action="primary"]') {
        add(this.primary);
      } else if (part === "[data-anvil-action]") {
        all.filter((el) => el.attrs["data-anvil-action"]).forEach(add);
      } else if (part.startsWith("[data-anvil-action")) {
        continue;
      } else if (part === "[data-anvil-state]") {
        add(this.main);
      } else if (part.startsWith("[contenteditable")) {
        all.filter(isEditable).forEach(add);
      } else if (part === "button" || part === "[role=button]" || part === "input[type=button]" || part === "input[type=submit]") {
        add(this.primary);
      } else if (part.startsWith("textarea") || part.startsWith("input")) {
        all.filter(isTextarea).forEach(add);
      }
    }
    return all.filter((el) => out.includes(el));
  }

  locator(selector) {
    return new FakeLocator(this, selector, this.querySelectorAll(selector));
  }

  async evaluate(fn, arg) {
    // The token was already on the page before the probe typed it: the first time
    // the probe looks for it, it is put there.
    if (cfg.tokenBeforeEntry && !this.preseededToken && typeof arg === "string" && arg.startsWith("anvil-")) {
      this.preseededToken = arg;
    }
    if (typeof arg === "string" && arg.startsWith("anvil-")) this.learnToken(arg);
    this.installGlobals();
    return fn(arg);
  }

  url() {
    const path = cfg.shape === "content_g" && this.route === "editor" ? `/notes/${this.editingId}` : "/";
    return `${this.origin}${path}${this.urlSuffix}`;
  }

  async goto(target) {
    this.origin = new URL(target).origin;
    this.load();
    return { status: () => 200 };
  }

  async reload() {
    if (cfg.reloadThrows && this.typed) throw new Error("page.reload: net::ERR_CONNECTION_RESET");
    this.reloadCount = (this.reloadCount || 0) + (this.typed ? 1 : 0);
    this.lastEditedId = this.editingId || this.lastEditedId;
    this.newEntry();
    this.activeElement = null;
    this.load();
    // A save answer that took longer than the probe waited arrives after the reload.
    for (const finish of this.heldUntilReload.splice(0)) setTimeout(finish, 0);
    return { status: () => 200 };
  }

  async waitForTimeout(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  viewportSize() {
    return { width: 1280, height: 720 };
  }
}

module.exports = {
  chromium: {
    launch: async () => ({
      newPage: async () => new FakePage(),
      close: async () => {}
    })
  }
};
