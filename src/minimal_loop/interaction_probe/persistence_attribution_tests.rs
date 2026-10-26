//! A contract-mode persistence verdict must come from the keys a commit changed,
//! never from the typed token surviving a reload: a search term or an autosaved
//! draft that outlives the reload is only a copy of what the probe typed. The
//! heuristic probe keeps its typed-token verdict, but text inside a text entry
//! control never counts as an echo after the reload. Every scenario runs the real
//! probe script against a fake Playwright page whose state is stored on reload
//! and whose effects follow the probe's own clicks and keystrokes, never the
//! wall clock.

use std::path::Path;

use serde_json::Value;

use super::tests::{FAKE_PROBE_SCENARIO_BUDGET, node_available, spawn_probe_http_server};
use super::{
    BrowserInteractionObservation, BrowserInteractionProbeOptions, PlaywrightResolution,
    probe_browser_interaction_against_running_server_with_options,
    write_test_availability_override_with_resolution,
};

const FAKE_PERSISTENCE_PLAYWRIGHT: &str = r#"const scenario = "__SCENARIO__";
const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const contractMode = scenario !== "heuristic_textarea_echo";

class FakeElement {
  constructor(page, kind, text = "") {
    this.page = page;
    this.kind = kind;
    this.attrs = {};
    this.disabled = false;
    this.value = "";
    this._textContent = text;
    this.tagName = kind === "textarea" ? "TEXTAREA" : kind === "button" ? "BUTTON" : "DIV";
    this.nodeType = ELEMENT_NODE;
  }

  get textContent() {
    return this.kind === "body" ? this.innerText : this._textContent;
  }

  get innerText() {
    if (this.kind !== "body") return this.textContent || this.value || "";
    return this.page.textNodes().map((node) => node.nodeValue).join(" ");
  }

  getAttribute(name) {
    if (this.kind === "state" && name === "data-anvil-state") {
      return JSON.stringify(this.page.snapshot());
    }
    return Object.prototype.hasOwnProperty.call(this.attrs, name) ? this.attrs[name] : null;
  }

  setAttribute(name, value) {
    this.attrs[name] = String(value);
  }

  contains(node) {
    return node === this || (node && node.parentElement === this);
  }

  getBoundingClientRect() {
    if (this.kind === "button") return { left: 500, top: 300, right: 620, bottom: 340, width: 120, height: 40 };
    if (this.kind === "textarea") return { left: 400, top: 380, right: 720, bottom: 460, width: 320, height: 80 };
    return { left: 0, top: 0, right: 20, bottom: 20, width: 20, height: 20 };
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
    if (this.elements[0].disabled) throw new Error(`element is not enabled for ${this.selector}`);
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

// What the app keeps across a reload, as localStorage would.
const stored = {};

class FakePage {
  constructor() {
    this.root = new FakeElement(this, "root");
    this.body = new FakeElement(this, "body");
    this.state = new FakeElement(this, "state");
    this.textarea = new FakeElement(this, "textarea");
    this.primary = new FakeElement(this, "button", "Save");
    if (contractMode) this.primary.setAttribute("data-anvil-action", "primary");
    this.activeElement = null;
    this.unsaved = false;
    this.restored = false;
    this.s = this.initialState();
    Object.assign(this.s, stored);
    this.keyboard = {
      type: async (text) => {
        if (this.activeElement === this.textarea) this.typeInto(this.textarea.value + text);
      },
      down: async () => {},
      up: async () => {},
      press: async () => {}
    };
    this.mouse = {
      click: async () => {}
    };
  }

  initialState() {
    if (scenario === "contract_search") return { search: "", visibleCount: 5 };
    if (scenario === "contract_todo") return { draft: "", todos: [] };
    if (scenario === "contract_editor_saved") return { content: "", contentLength: 0, savedAt: 0 };
    if (scenario === "contract_editor_draft_copy") return { content: "", contentLength: 0 };
    return { draft: "" };
  }

  typeInto(value) {
    this.textarea.value = value;
    this.unsaved = true;
    this.restored = false;
    if (scenario === "contract_search") {
      this.s.search = value;
      this.s.visibleCount = value ? 1 : 5;
      this.persist(["search", "visibleCount"]);
    } else if (scenario === "contract_todo") {
      this.s.draft = value;
    } else if (scenario === "heuristic_textarea_echo") {
      this.s.draft = value;
      this.persist(["draft"]);
    } else {
      this.s.content = value;
      this.s.contentLength = value.length;
      this.persist(["content", "contentLength"]);
    }
  }

  persist(keys) {
    for (const key of keys) stored[key] = this.s[key];
  }

  clickElement(el) {
    if (el !== this.primary) return;
    if (scenario === "contract_todo" && this.s.draft) {
      this.s.todos = [...this.s.todos, this.s.draft];
      this.s.draft = "";
      this.persist(["todos"]);
    }
    if (scenario === "contract_editor_saved" && this.unsaved) {
      this.s.savedAt += 1;
      this.persist(["savedAt"]);
    }
    this.unsaved = false;
  }

  snapshot() {
    return JSON.parse(JSON.stringify(this.s));
  }

  buttons() {
    return [this.primary];
  }

  installGlobals() {
    global.window = {
      innerWidth: 1280,
      innerHeight: 720,
      getComputedStyle: () => ({ visibility: "visible", display: "block", opacity: "1" })
    };
    global.document = new FakeDocument(this);
    global.Node = { ELEMENT_NODE, TEXT_NODE };
    global.NodeFilter = { SHOW_TEXT: 4 };
  }

  querySelectorAll(selector) {
    const out = [];
    const add = (el) => {
      if (el && !out.includes(el)) out.push(el);
    };
    for (const raw of String(selector || "").split(",")) {
      const part = raw.trim();
      if (!part) continue;
      if (part === "*") {
        [this.root, this.body, ...(contractMode ? [this.state] : []), this.primary, this.textarea].forEach(add);
        continue;
      }
      if (part === "[data-anvil-probe-text-target=\"1\"]" || part === "[data-anvil-probe-text-target='1']") {
        if (this.textarea.getAttribute("data-anvil-probe-text-target") === "1") add(this.textarea);
        continue;
      }
      if (part === '[data-anvil-action="primary"]' || part === "[data-anvil-action]") {
        if (contractMode) add(this.primary);
        continue;
      }
      if (part.startsWith("[data-anvil-action")) continue;
      if (part.includes("[data-anvil-state]") && contractMode) add(this.state);
      if (part.includes("button") || part.includes("[role=button]") || part.includes("input[type=button]") || part.includes("input[type=submit]")) {
        add(this.primary);
      }
      if (
        part.includes("textarea") ||
        (part.startsWith("input") && !part.startsWith("input[type=button]") && !part.startsWith("input[type=submit]")) ||
        part.includes("select")
      ) {
        add(this.textarea);
      }
    }
    return out;
  }

  textNodes() {
    const nodes = [];
    const push = (value, parent) => {
      if (value) nodes.push(new FakeTextNode(value, parent));
    };
    push("Notes", this.body);
    if (scenario === "contract_search") {
      push(this.s.search ? `Results for ${this.s.search}` : "Results", this.body);
      push(`${this.s.visibleCount} shown`, this.body);
    } else if (scenario === "contract_todo") {
      push(`Todos ${this.s.todos.length}`, this.body);
      for (const todo of this.s.todos) push(todo, this.body);
    } else if (scenario === "heuristic_textarea_echo") {
      // Before the reload the draft is echoed in a preview; after it, the token is
      // only the default text of the restored textarea.
      if (this.restored) push(stored.draft, this.textarea);
      else push(this.s.draft ? `Preview ${this.s.draft}` : "", this.body);
    } else {
      push(this.s.content ? `Preview ${this.s.content}` : "", this.body);
    }
    return nodes;
  }

  locator(selector) {
    return new FakeLocator(this, selector, this.querySelectorAll(selector));
  }

  on() {}

  async evaluate(fn, arg) {
    this.installGlobals();
    return fn(arg);
  }

  async goto() {
    return { status: () => 200 };
  }

  async reload() {
    // A reload builds new elements, so the probe's own text target mark is gone.
    this.textarea = new FakeElement(this, "textarea");
    this.activeElement = null;
    this.restored = true;
    this.s = this.initialState();
    Object.assign(this.s, stored);
    if (scenario === "heuristic_textarea_echo") this.s.draft = "";
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
"#;

fn install_persistence_playwright(root: &Path, scenario: &str) -> PlaywrightResolution {
    let node_modules = root.join("node_modules");
    let playwright_dir = node_modules.join("playwright");
    std::fs::create_dir_all(&playwright_dir).unwrap();
    std::fs::write(
        playwright_dir.join("package.json"),
        "{\"version\":\"0.0.0-fake\"}\n",
    )
    .unwrap();
    std::fs::write(
        playwright_dir.join("index.js"),
        FAKE_PERSISTENCE_PLAYWRIGHT.replace("__SCENARIO__", scenario),
    )
    .unwrap();
    PlaywrightResolution {
        module_path: playwright_dir.join("index.js").display().to_string(),
        module_dir: playwright_dir.display().to_string(),
        node_path: Some(node_modules.display().to_string()),
        location: "fake_playwright".to_string(),
        version: "0.0.0-fake".to_string(),
    }
}

/// Runs the real probe with persistence, text entry and token echo required and
/// returns the observation with the evidence JSON it wrote, or `None` when node
/// or loopback networking is unavailable.
fn run_persistence_scenario(scenario: &str) -> Option<(BrowserInteractionObservation, Value)> {
    if !node_available() {
        eprintln!("skipping persistence attribution scenario because node is unavailable");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let resolution = install_persistence_playwright(dir.path(), scenario);
    write_test_availability_override_with_resolution(dir.path(), true, Some(&resolution));
    let (port, server) = spawn_probe_http_server()?;
    let run_dir = dir.path().join(".anvil/runs/fake-playwright");
    let path = run_dir.join("browser-interaction.json");
    let outcome = probe_browser_interaction_against_running_server_with_options(
        dir.path(),
        port,
        &run_dir,
        &path,
        FAKE_PROBE_SCENARIO_BUDGET,
        BrowserInteractionProbeOptions {
            persistence_required: true,
            text_entry_required: true,
            token_echo_required: true,
        },
    );
    let _ = server.join();
    let observation = outcome.observation().cloned()?;
    let evidence = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    Some((observation, evidence))
}

fn has_kind(observation: &BrowserInteractionObservation, kind: &str) -> bool {
    observation
        .informational_failure_kinds
        .iter()
        .any(|candidate| candidate == kind)
}

/// Only the typed draft survived: persistence is unverified, the run fails
/// honestly, and the draft-only kind is reported.
fn assert_draft_only_run(scenario: &str) {
    let Some((observation, evidence)) = run_persistence_scenario(scenario) else {
        return;
    };
    assert_eq!(
        observation.probe_mode, "contract",
        "{scenario}: {observation:?}"
    );
    assert!(
        observation.token_echoed,
        "{scenario}: the token must be echoed so the old typed-token branch applies: {observation:?}"
    );
    assert_eq!(
        observation.persistence_after_reload, "not_evaluated",
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.persistence_after_reload_reason, "no_mutation_observed",
        "{scenario}: {observation:?}"
    );
    assert!(
        observation.persistence_changed_dimensions.is_empty(),
        "{scenario}: {observation:?}"
    );
    assert!(
        has_kind(&observation, "persistence_draft_only"),
        "{scenario}: {observation:?}"
    );
    assert!(!observation.ok, "{scenario}: {observation:?}");
    assert_eq!(
        observation.failure_kind, "persistence_not_evaluated:no_mutation_observed",
        "{scenario}: {observation:?}"
    );
    assert_eq!(evidence["persistence_after_reload"], "not_evaluated");
    assert_eq!(evidence["ok"], false);
}

#[test]
fn contract_search_term_that_survives_reload_is_draft_only_not_preserved() {
    assert_draft_only_run("contract_search");
}

#[test]
fn contract_todo_added_by_the_primary_action_is_preserved_by_its_commit_key() {
    let Some((observation, evidence)) = run_persistence_scenario("contract_todo") else {
        return;
    };
    assert_eq!(observation.probe_mode, "contract", "{observation:?}");
    assert_eq!(
        observation.persistence_after_reload, "preserved",
        "{observation:?}"
    );
    assert_eq!(
        observation.persistence_changed_dimensions,
        vec!["todos".to_string()],
        "{observation:?}"
    );
    assert!(
        !has_kind(&observation, "persistence_draft_only"),
        "{observation:?}"
    );
    assert_eq!(evidence["persistence_after_reload"], "preserved");
}

#[test]
fn contract_editor_save_key_that_survives_reload_is_preserved_by_the_commit_key() {
    let Some((observation, evidence)) = run_persistence_scenario("contract_editor_saved") else {
        return;
    };
    assert_eq!(observation.probe_mode, "contract", "{observation:?}");
    assert_eq!(
        observation.persistence_after_reload, "preserved",
        "{observation:?}"
    );
    assert_eq!(
        observation.persistence_changed_dimensions,
        vec!["savedAt".to_string()],
        "{observation:?}"
    );
    assert!(
        !has_kind(&observation, "persistence_draft_only"),
        "{observation:?}"
    );
    assert_eq!(evidence["persistence_after_reload"], "preserved");
}

#[test]
fn contract_editor_without_a_save_key_is_draft_only_not_preserved() {
    assert_draft_only_run("contract_editor_draft_copy");
}

#[test]
fn heuristic_token_found_only_inside_the_textarea_after_reload_is_not_an_echo() {
    let Some((observation, evidence)) = run_persistence_scenario("heuristic_textarea_echo") else {
        return;
    };
    assert_eq!(observation.probe_mode, "heuristic", "{observation:?}");
    assert!(observation.token_echoed, "{observation:?}");
    assert_eq!(
        observation.persistence_after_reload, "reset",
        "{observation:?}"
    );
    assert!(!observation.ok, "{observation:?}");
    assert_eq!(
        observation.failure_kind, "persistence_after_reload_reset",
        "{observation:?}"
    );
    assert_eq!(evidence["persistence_after_reload"], "reset");
}
