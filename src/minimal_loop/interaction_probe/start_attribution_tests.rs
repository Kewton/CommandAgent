//! A contract-mode run whose start was never observed must not borrow the legacy
//! recovery to invent one: a button the probe happens to press that moves the
//! marker is not evidence that the contract's start works. Heuristic mode keeps the
//! legacy promotion. Every scenario runs the real probe script against a fake
//! Playwright page whose timed effects follow the probe's own reads and clicks,
//! never the wall clock.

use std::path::Path;

use serde_json::Value;

use super::tests::{FAKE_PROBE_SCENARIO_BUDGET, node_available, spawn_probe_http_server};
use super::{
    BrowserInteractionObservation, BrowserInteractionProbeOptions, PlaywrightResolution,
    probe_browser_interaction_against_running_server_with_options,
    write_test_availability_override_with_resolution,
};

const FAKE_START_PLAYWRIGHT: &str = r#"const scenario = "__SCENARIO__";
const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const contractMode = scenario !== "heuristic_recovery";

class FakeElement {
  constructor(page, kind, text = "", big = true) {
    this.page = page;
    this.kind = kind;
    this.attrs = {};
    this.disabled = false;
    this.value = "";
    this.big = big;
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
      this.page.onStateRead();
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
    if (this.kind === "button" && this.big) return { left: 500, top: 300, right: 620, bottom: 340, width: 120, height: 40 };
    if (this.kind === "button") return { left: 500, top: 300, right: 560, bottom: 320, width: 60, height: 20 };
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
    this.title = "Fake game";
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

  async fill(value) {
    if (!this.elements.length) throw new Error(`no element for ${this.selector}`);
    this.elements[0].value = value;
    this.page.s.draft = value;
  }

  async boundingBox() {
    if (!this.elements.length) return null;
    const box = this.elements[0].getBoundingClientRect();
    return { x: box.left, y: box.top, width: box.width, height: box.height };
  }
}

class FakePage {
  constructor() {
    this.s = { status: "menu", pos: 0, tick: 0, draft: "" };
    // Timed effects follow the probe's reads and clicks, so machine load cannot move them.
    this.tickArmed = false;
    this.startPending = false;
    this.startArmed = false;
    this.root = new FakeElement(this, "root");
    this.body = new FakeElement(this, "body");
    this.state = new FakeElement(this, "state");
    this.textarea = new FakeElement(this, "textarea");
    this.activeElement = null;
    this.primary = null;
    this.restart = null;
    this.targets = [];
    this.others = [];
    const add = (text, big) => new FakeElement(this, "button", text, big);
    if (scenario === "startless_form") {
      this.primary = add("Submit", true);
    } else if (scenario === "unstarted_restart_hook") {
      // A restart hook exists before any start, as in a game that needs a restart.
      this.primary = add("Start", true);
      this.targets = [add("Go", false)];
      this.restart = add("Restart", true);
      this.restart.setAttribute("data-anvil-action", "restart");
      this.others = [this.restart, add("Tap", true), add("Tap", true), add("Tap", true)];
    } else if (scenario === "delayed_primary") {
      this.primary = add("Start", true);
      this.others = [add("Tap", true), add("Tap", true), add("Tap", true), add("Tap", true)];
    } else if (scenario === "heuristic_recovery") {
      // The small button ranks fifth, so only the legacy recovery reaches it.
      this.targets = [add("Go", false)];
      this.others = [add("Tap", true), add("Tap", true), add("Tap", true), add("Tap", true)];
    } else {
      this.primary = add("Start", true);
      this.targets = [add("Go", false)];
      this.others = [add("Tap", true), add("Tap", true), add("Tap", true), add("Tap", true)];
    }
    if (this.primary) this.primary.setAttribute("data-anvil-action", "primary");
    this.keyboard = {
      type: async (text) => {
        if (this.activeElement === this.textarea) {
          this.textarea.value += text;
          this.s.draft = this.textarea.value;
        }
      },
      down: async (key) => {
        if (key === "ArrowLeft") this.s.pos -= 1;
      },
      up: async () => {},
      press: async () => {}
    };
    this.mouse = {
      click: async () => {}
    };
  }

  onStateRead() {
    if (this.startArmed) {
      this.s.status = "playing";
      this.startArmed = false;
      this.startPending = false;
    }
    if (this.tickArmed) this.s.tick += 1;
  }

  snapshot() {
    return { ...this.s };
  }

  buttons() {
    const out = [];
    if (this.primary) out.push(this.primary);
    return out.concat(this.targets, this.others);
  }

  hasTextarea() {
    return scenario === "startless_form";
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
        [this.root, this.body, ...(contractMode ? [this.state] : []), ...this.buttons()].forEach(add);
        if (this.hasTextarea()) add(this.textarea);
        continue;
      }
      if (part === "[data-anvil-probe-text-target=\"1\"]" || part === "[data-anvil-probe-text-target='1']") {
        if (this.textarea.getAttribute("data-anvil-probe-text-target") === "1") add(this.textarea);
        continue;
      }
      if (part === '[data-anvil-action="primary"]') {
        add(this.primary);
        continue;
      }
      if (part === '[data-anvil-action="restart"]') {
        add(this.restart);
        continue;
      }
      if (part === "[data-anvil-action]") {
        add(this.primary);
        add(this.restart);
        continue;
      }
      if (part.startsWith("[data-anvil-action")) continue;
      if (part.includes("[data-anvil-state]") && contractMode) add(this.state);
      if (part.includes("button") || part.includes("[role=button]") || part.includes("input[type=button]") || part.includes("input[type=submit]")) {
        this.buttons().forEach(add);
      }
      if (
        part.includes("textarea") ||
        (part.startsWith("input") && !part.startsWith("input[type=button]") && !part.startsWith("input[type=submit]")) ||
        part.includes("select")
      ) {
        if (this.hasTextarea()) add(this.textarea);
      }
    }
    return out;
  }

  textNodes() {
    const nodes = [];
    const push = (value, parent) => {
      if (value) nodes.push(new FakeTextNode(value, parent));
    };
    push(this.s.status, this.body);
    push(`pos ${this.s.pos}`, this.body);
    return nodes;
  }

  clickElement(el) {
    if (el === this.primary) {
      // Only the delayed start reacts to the primary press, and only once the probe
      // clicks something else afterwards (see onStateRead).
      if (scenario === "delayed_primary" && this.s.status === "menu") this.startPending = true;
      return;
    }
    if (this.startPending) this.startArmed = true;
    if (this.targets.includes(el)) {
      if (scenario === "unstarted_tick") this.tickArmed = true;
      else this.s.status = "playing";
    }
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

fn install_start_playwright(root: &Path, scenario: &str) -> PlaywrightResolution {
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
        FAKE_START_PLAYWRIGHT.replace("__SCENARIO__", scenario),
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

/// Runs the real probe and returns the observation with the evidence JSON it
/// wrote, or `None` when node or loopback networking is unavailable.
fn run_start_scenario(scenario: &str) -> Option<(BrowserInteractionObservation, Value)> {
    if !node_available() {
        eprintln!("skipping start attribution scenario because node is unavailable");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let resolution = install_start_playwright(dir.path(), scenario);
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
        BrowserInteractionProbeOptions::default(),
    );
    let _ = server.join();
    let observation = outcome.observation().cloned()?;
    let evidence = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    Some((observation, evidence))
}

fn has_step(observation: &BrowserInteractionObservation, step: &str) -> bool {
    observation.steps.iter().any(|candidate| candidate == step)
}

/// The start was never observed: the run fails honestly and the legacy recovery
/// neither runs nor leaves a start (or recovery) transition behind.
fn assert_unstarted_contract_run(scenario: &str) -> Option<BrowserInteractionObservation> {
    let (observation, evidence) = run_start_scenario(scenario)?;
    assert_eq!(
        observation.probe_mode, "contract",
        "{scenario}: {observation:?}"
    );
    assert!(!observation.ok, "{scenario}: {observation:?}");
    assert_eq!(
        observation.failure_kind, "start_transition_missing",
        "{scenario}: {observation:?}"
    );
    assert!(
        !has_step(&observation, "start_transition"),
        "{scenario}: {observation:?}"
    );
    assert!(
        !has_step(&observation, "recovery_transition"),
        "{scenario}: {observation:?}"
    );
    assert!(
        has_step(&observation, "recovery_transition:not_observed"),
        "{scenario}: {observation:?}"
    );
    assert!(
        !observation.recovery_transition_observed && observation.recovery_transition_not_observed,
        "{scenario}: {observation:?}"
    );
    assert_eq!(observation.recovery_control, "none", "{scenario}");
    assert_eq!(evidence["recovery_transition_status"], "not_observed");
    assert_eq!(evidence["recovery_control"], "none");
    assert_eq!(evidence["start_transition"], false);
    assert_eq!(evidence["ok"], false);
    Some(observation)
}

#[test]
fn contract_unstarted_run_does_not_promote_a_fifth_button_to_the_start() {
    assert_unstarted_contract_run("unstarted_state");
}

#[test]
fn contract_unstarted_run_does_not_promote_a_self_moving_state_to_the_start() {
    assert_unstarted_contract_run("unstarted_tick");
}

#[test]
fn contract_unstarted_run_with_a_restart_hook_reports_no_recovery_and_no_start() {
    let Some(observation) = assert_unstarted_contract_run("unstarted_restart_hook") else {
        return;
    };
    assert!(
        observation
            .action_hooks
            .iter()
            .any(|hook| hook == "restart"),
        "{observation:?}"
    );
    assert!(!observation.restart_hook_reachable_after_start);
}

#[test]
fn contract_start_landing_inside_the_candidate_window_is_still_observed() {
    let Some((observation, evidence)) = run_start_scenario("delayed_primary") else {
        return;
    };
    assert_eq!(observation.probe_mode, "contract", "{observation:?}");
    assert!(observation.ok, "{observation:?}");
    assert!(
        has_step(&observation, "start_transition"),
        "{observation:?}"
    );
    assert!(
        !has_step(&observation, "recovery_transition"),
        "{observation:?}"
    );
    assert!(!observation.primary_transition_observed, "{observation:?}");
    assert_eq!(evidence["start_transition"], true);
}

#[test]
fn contract_form_without_a_start_control_is_still_startless_ok() {
    let Some((observation, evidence)) = run_start_scenario("startless_form") else {
        return;
    };
    assert_eq!(observation.probe_mode, "contract", "{observation:?}");
    assert!(observation.ok, "{observation:?}");
    assert!(!observation.start_control_found, "{observation:?}");
    assert!(
        !has_step(&observation, "start_transition"),
        "{observation:?}"
    );
    assert_eq!(evidence["start_transition"], false);
    assert_eq!(evidence["recovery_transition_status"], "not_observed");
    assert_eq!(evidence["recovery_control"], "none");
}

#[test]
fn heuristic_run_still_promotes_the_legacy_recovery_to_the_start() {
    let Some((observation, evidence)) = run_start_scenario("heuristic_recovery") else {
        return;
    };
    assert_eq!(observation.probe_mode, "heuristic", "{observation:?}");
    assert!(observation.ok, "{observation:?}");
    assert!(
        has_step(&observation, "recovery_transition"),
        "{observation:?}"
    );
    assert!(
        has_step(&observation, "start_transition"),
        "{observation:?}"
    );
    assert!(observation.recovery_transition_observed, "{observation:?}");
    assert_eq!(evidence["recovery_transition_status"], "observed");
    assert_eq!(evidence["start_transition"], true);
}
