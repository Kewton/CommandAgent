//! Contract-mode recovery must be attributable to a restart control (hook or
//! `r` key) and move the state back toward the post-start baseline. A primary
//! press, a value that settles or ticks by itself, or a keystroke that lands in
//! a focused input is never recovery. Every scenario here runs the real probe
//! script against a fake Playwright page.

use std::path::Path;

use serde_json::Value;

use super::tests::{FAKE_PROBE_SCENARIO_BUDGET, node_available, spawn_probe_http_server};
use super::{
    BrowserInteractionObservation, BrowserInteractionProbeOptions, PlaywrightResolution,
    probe_browser_interaction_against_running_server_with_options,
    write_test_availability_override_with_resolution,
};

const FAKE_RECOVERY_PLAYWRIGHT: &str = r#"const scenario = "__SCENARIO__";
const ELEMENT_NODE = 1;
const TEXT_NODE = 3;

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

  set textContent(value) {
    this._textContent = String(value || "");
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

  blur() {
    if (this.page.activeElement === this) this.page.activeElement = null;
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
    this.s = {
      status: "menu",
      score: 0,
      lives: 3,
      shake: false,
      tick: 0,
      playerX: 300,
      draft: "",
      loading: false,
      toast: "",
      cooldown: 0,
      angle: 0
    };
    this.cooldownTimer = null;
    this.tickPausedUntil = 0;
    this.vx = 0;
    this.inertia = null;
    this.rPresses = 0;
    this.root = new FakeElement(this, "root");
    this.body = new FakeElement(this, "body");
    this.primary = new FakeElement(this, "button", "Start");
    this.primary.setAttribute("data-anvil-action", "primary");
    this.restart = new FakeElement(this, "button", "Restart");
    this.restart.setAttribute("data-anvil-action", "restart");
    this.restart.disabled = scenario === "restart_blocked_focus";
    this.textarea = new FakeElement(this, "textarea");
    this.state = new FakeElement(this, "state");
    this.activeElement = null;
    this.keyboard = {
      type: async (text) => {
        if (this.activeElement === this.textarea) {
          this.textarea.value += text;
          this.s.draft = this.textarea.value;
        }
      },
      down: async (key) => {
        if (scenario === "restart_inertia" && key === "ArrowLeft" && this.s.status === "playing") {
          this.vx = -8;
          this.startInertia();
        }
        const lethal = [
          "restart_gameover",
          "restart_key",
          "primary_restart_combo",
          "restart_late_hook",
          "restart_async_reset",
          "restart_tick_lag_reset",
          "restart_noop_unrelated_return",
          "restart_transient_baseline"
        ].includes(scenario);
        // The ticking game changes on its own, so its first key must already end the run.
        const lethalKey = key === "Space" || scenario === "restart_tick_lag_reset";
        if (lethal && lethalKey && this.s.status === "playing") {
          this.s.score = 50;
          this.s.lives = 0;
          this.s.status = "gameover";
          if (scenario === "restart_noop_unrelated_return") {
            setTimeout(() => {
              this.s.toast = "msg";
            }, 100);
          }
        }
        if (scenario.startsWith("restart_noop_cooldown") && key === "Space" && this.s.status === "playing") {
          const lastStep = scenario === "restart_noop_cooldown_last_step";
          const slow = scenario === "restart_noop_cooldown_slow" || lastStep;
          this.s.cooldown = lastStep ? 2 : slow ? 3 : scenario === "restart_noop_cooldown" ? 60 : 14;
          const step = () => {
            if (this.s.cooldown > 0) this.s.cooldown -= 1;
          };
          if (!this.cooldownTimer) {
            // The slow cooldown takes its first step while the probe watches, then one per second.
            if (slow) {
              setTimeout(() => {
                step();
                this.cooldownTimer = setInterval(step, 1000);
              }, 300);
              this.cooldownTimer = true;
            } else {
              this.cooldownTimer = setInterval(step, 100);
            }
          }
        }
      },
      up: async () => {},
      press: async (key) => {
        if (key !== "r") return;
        this.rPresses += 1;
        if (scenario === "restart_key" && this.s.status === "gameover") this.resetRun();
        if (scenario === "restart_key_window_handler") {
          // The page's window keydown handler ignores keys typed inside an input.
          if (this.activeElement === this.textarea) {
            this.textarea.value += "r";
            this.s.draft = this.textarea.value;
          } else {
            this.textarea.value = "";
            this.s.draft = "";
          }
        }
        if (scenario === "restart_blocked_focus" && this.activeElement === this.textarea) {
          this.textarea.value += "r";
          this.s.draft = this.textarea.value;
        }
      }
    };
    this.mouse = {
      click: async () => {}
    };
  }

  snapshot() {
    const s = this.s;
    switch (scenario) {
      case "primary_shake": return { status: s.status, score: s.score, shake: s.shake };
      case "menu_only_restart": return { status: s.status, score: s.score };
      case "restart_noop_tick": return { status: s.status, tick: s.tick };
      case "restart_blocked_focus":
      case "restart_key_window_handler": return { status: s.status, draft: s.draft };
      case "primary_late_return": return { status: s.status, score: s.score };
      case "menu_restart_late_return": return { status: s.status, score: s.score };
      case "restart_async_reset": return { status: s.status, lives: s.lives, score: s.score, loading: s.loading };
      case "restart_tick_lag_reset": return { status: s.status, lives: s.lives, score: s.score, tick: s.tick };
      case "restart_noop_unrelated_return": return { status: s.status, lives: s.lives, score: s.score, toast: s.toast };
      case "restart_noop_cooldown":
      case "restart_noop_cooldown_slow":
      case "restart_noop_cooldown_expiring": return { status: s.status, cooldown: s.cooldown };
      case "restart_noop_cooldown_last_step": return { status: s.status, cooldown: s.cooldown };
      case "restart_tick_jump_reset": return { status: s.status, tick: s.tick };
      case "restart_noop_angle_wrap": return { status: s.status, angle: s.angle };
      case "restart_inertia": return { status: s.status, playerX: s.playerX };
      default: return { status: s.status, lives: s.lives, score: s.score };
    }
  }

  restartPresent() {
    const hookless = ["primary_shake", "restart_key", "primary_late_return", "primary_restart_combo", "restart_key_window_handler"];
    if (hookless.includes(scenario)) return false;
    if (scenario === "restart_late_hook") return this.rPresses > 0;
    if (scenario === "menu_only_restart" || scenario === "menu_restart_late_return") return this.s.status === "menu";
    return true;
  }

  hasTextarea() {
    return scenario === "restart_blocked_focus" || scenario === "restart_key_window_handler";
  }

  buttons() {
    return this.restartPresent() ? [this.primary, this.restart] : [this.primary];
  }

  resetRun() {
    this.s.status = "playing";
    this.s.lives = 3;
    this.s.score = 0;
  }

  startInertia() {
    if (this.inertia) return;
    this.inertia = setInterval(() => {
      this.s.playerX += Math.round(this.vx);
      this.vx *= 0.8;
      if (Math.abs(this.vx) < 0.5) this.stopInertia();
    }, 40);
  }

  stopInertia() {
    if (this.inertia) clearInterval(this.inertia);
    this.inertia = null;
    this.vx = 0;
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
        [this.root, this.body, this.state, ...this.buttons()].forEach(add);
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
        if (this.restartPresent()) add(this.restart);
        continue;
      }
      if (part === "[data-anvil-action]") {
        this.buttons().forEach(add);
        continue;
      }
      if (part.startsWith("[data-anvil-action")) continue;
      if (part.includes("[data-anvil-state]")) add(this.state);
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
    push(this.primary.textContent, this.primary);
    push(this.s.status, this.body);
    return nodes;
  }

  clickElement(el) {
    if (el === this.restart) {
      if (scenario === "restart_gameover" || scenario === "restart_late_hook") this.resetRun();
      if (scenario === "restart_inertia") {
        this.stopInertia();
        this.s.playerX = 300;
      }
      if (scenario === "restart_tick_jump_reset") {
        // The game shows a two-second countdown after a restart, so the counter holds still.
        this.s.tick = 0;
        this.tickPausedUntil = Date.now() + 2000;
      }
      if (scenario === "restart_transient_baseline") {
        setTimeout(() => {
          this.s.score = 0;
        }, 100);
        setTimeout(() => {
          this.s.score = 50;
        }, 300);
      }
      if (scenario === "restart_async_reset") {
        this.s.loading = true;
        setTimeout(() => {
          this.resetRun();
          this.s.loading = false;
        }, 300);
      }
      if (scenario === "restart_tick_lag_reset") {
        setTimeout(() => this.resetRun(), 100);
      }
      if (scenario === "restart_noop_unrelated_return") {
        setTimeout(() => {
          this.s.toast = "";
        }, 500);
      }
      return;
    }
    if (el !== this.primary) return;
    if (this.s.status === "menu") {
      this.s.status = "playing";
      if (["restart_noop_tick", "restart_tick_lag_reset", "restart_tick_jump_reset"].includes(scenario)) {
        setInterval(() => {
          if (Date.now() >= this.tickPausedUntil) this.s.tick += 1;
        }, 100);
      }
      if (scenario === "restart_noop_angle_wrap") {
        setInterval(() => {
          this.s.angle = (this.s.angle + 90) % 360;
        }, 100);
      }
      return;
    }
    if (scenario === "primary_restart_combo") {
      // One "Start / Restart" button: pressing it again resets the run.
      this.resetRun();
    }
    if (["primary_shake", "menu_only_restart", "primary_late_return", "menu_restart_late_return"].includes(scenario)) {
      this.s.score += 10;
    }
    if (scenario === "primary_late_return" || scenario === "menu_restart_late_return") {
      setTimeout(() => {
        this.s.score = 0;
      }, 500);
    }
    if (scenario === "primary_shake") {
      this.s.shake = true;
      setTimeout(() => {
        this.s.shake = false;
      }, 300);
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

fn install_recovery_playwright(root: &Path, scenario: &str) -> PlaywrightResolution {
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
        FAKE_RECOVERY_PLAYWRIGHT.replace("__SCENARIO__", scenario),
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
fn run_recovery_scenario(scenario: &str) -> Option<(BrowserInteractionObservation, Value)> {
    if !node_available() {
        eprintln!("skipping recovery attribution scenario because node is unavailable");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let resolution = install_recovery_playwright(dir.path(), scenario);
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

fn assert_recovery(scenario: &str, observed: bool, control: &str) {
    let Some((observation, evidence)) = run_recovery_scenario(scenario) else {
        return;
    };
    assert_eq!(
        observation.probe_mode, "contract",
        "{scenario}: {observation:?}"
    );
    assert!(
        observation
            .steps
            .iter()
            .any(|step| step == "start_transition"),
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.recovery_transition_observed, observed,
        "{scenario}: {observation:?}"
    );
    assert_eq!(
        observation.recovery_transition_not_observed, !observed,
        "{scenario}: {observation:?}"
    );
    assert_eq!(observation.recovery_control, control, "{scenario}");
    let status = if observed { "observed" } else { "not_observed" };
    assert_eq!(evidence["recovery_transition_status"], status, "{scenario}");
    assert_eq!(evidence["recovery_control"], control, "{scenario}");
}

#[test]
fn primary_press_with_self_clearing_shake_is_not_recovery() {
    assert_recovery("primary_shake", false, "primary_fallback");
}

#[test]
fn menu_only_restart_with_primary_score_change_is_not_recovery() {
    assert_recovery("menu_only_restart", false, "primary_fallback");
}

#[test]
fn noop_restart_hook_with_ticking_value_is_not_recovery() {
    assert_recovery("restart_noop_tick", false, "restart");
}

#[test]
fn blocked_restart_with_focused_input_does_not_count_key_r_text() {
    assert_recovery("restart_blocked_focus", false, "restart");
}

#[test]
fn restart_hook_after_gameover_back_to_baseline_is_recovery() {
    assert_recovery("restart_gameover", true, "restart");
}

#[test]
fn keyboard_r_back_to_baseline_is_recovery() {
    assert_recovery("restart_key", true, "keyboard_r");
}

#[test]
fn restart_after_inertial_motion_back_to_baseline_is_recovery() {
    assert_recovery("restart_inertia", true, "restart");
}

#[test]
fn restart_hook_appearing_after_start_is_recovery() {
    assert_recovery("restart_late_hook", true, "restart");
}

#[test]
fn async_restart_reset_after_loading_is_recovery() {
    assert_recovery("restart_async_reset", true, "restart");
}

#[test]
fn restart_reset_lagging_one_tick_is_recovery() {
    assert_recovery("restart_tick_lag_reset", true, "restart");
}

#[test]
fn noop_restart_with_long_self_counting_cooldown_is_not_recovery() {
    assert_recovery("restart_noop_cooldown", false, "restart");
}

#[test]
fn noop_restart_with_cooldown_reaching_baseline_unaided_is_not_recovery() {
    assert_recovery("restart_noop_cooldown_expiring", false, "restart");
}

#[test]
fn primary_press_with_score_returning_to_baseline_later_is_not_recovery() {
    assert_recovery("primary_late_return", false, "primary_fallback");
}

#[test]
fn menu_only_restart_with_score_returning_to_baseline_later_is_not_recovery() {
    assert_recovery("menu_restart_late_return", false, "primary_fallback");
}

#[test]
fn noop_restart_with_unplayed_key_returning_to_baseline_later_is_not_recovery() {
    assert_recovery("restart_noop_unrelated_return", false, "restart");
}

#[test]
fn noop_restart_with_cooldown_stepping_once_per_second_is_not_recovery() {
    assert_recovery("restart_noop_cooldown_slow", false, "restart");
}

#[test]
fn restart_leaving_value_only_transiently_at_baseline_is_not_recovery() {
    assert_recovery("restart_transient_baseline", false, "restart");
}

#[test]
fn restart_jumping_ticking_counter_to_baseline_is_recovery() {
    assert_recovery("restart_tick_jump_reset", true, "restart");
}

#[test]
fn noop_restart_with_cooldown_reaching_baseline_by_one_step_is_not_recovery() {
    assert_recovery("restart_noop_cooldown_last_step", false, "restart");
}

#[test]
fn noop_restart_with_wrapping_angle_is_not_recovery() {
    assert_recovery("restart_noop_angle_wrap", false, "restart");
}

#[test]
fn primary_start_restart_button_resetting_state_is_not_counted_as_recovery() {
    assert_recovery("primary_restart_combo", false, "primary_fallback");
}

#[test]
fn key_r_restart_ignored_by_window_handler_while_input_focused_needs_blur() {
    assert_recovery("restart_key_window_handler", true, "keyboard_r");
}
