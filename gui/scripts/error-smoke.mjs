import { spawn } from "node:child_process";
import { chmod, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const guiRoot = resolve(scriptDirectory, "..");
const repositoryRoot = resolve(guiRoot, "..");
const managedPlaywrightPath =
  process.env.COMMANDAGENT_PLAYWRIGHT_PATH ??
  join(homedir(), ".anvil", "tools", "interaction-probe", "node_modules", "playwright");
const trialToken = "commandagent-gui-error-smoke-token-000000000072";
const scratchRoot = await mkdtemp(join(tmpdir(), "commandagent-gui-error-smoke-"));
const executionRoot = join(scratchRoot, "workspace");
const fakeCli = join(scratchRoot, "fake-commandagent.mjs");
const require = createRequire(import.meta.url);
const { chromium } = require(managedPlaywrightPath);

let browser;
let server;
try {
  await mkdir(join(executionRoot, "cli"), { recursive: true });
  await writeFile(join(executionRoot, "cli", "main.py"), "print('fixture')\n");
  await writeFile(
    fakeCli,
    `#!/usr/bin/env node
import { writeFileSync } from "node:fs";
setTimeout(() => {
  writeFileSync(
    process.env.COMMANDAGENT_EVAL_EVENTS,
    '{"event":"tui_command_stop","ok":false,"status":"failed","assurance_level":"partial"}\\n',
  );
}, 3000);
`,
  );
  await chmod(fakeCli, 0o755);

  await runChecked("npm", ["run", "build"], guiRoot, { ...process.env, GUI_BASE_PATH: "/" });
  server = await startServer(executionRoot, fakeCli);
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(new URL("/try/", server.origin).href, { waitUntil: "networkidle" });

  const tokenInput = page.locator("[data-testid='trial-token']");
  const checkContract = page.locator("[data-testid='check-contract']");
  const error = page.locator(".trial-compose > .trial-error[role='alert']");

  await page
    .locator("[data-testid='trial-goal']")
    .fill("Create a CLI --pattern filter command");
  await page.locator("[data-testid='trial-executor-model']").fill("fixture-executor");
  await page.locator("[data-testid='trial-planner-model']").fill("fixture-planner");
  const provider = await page.locator("[data-testid='trial-provider']").inputValue();
  const spec = {
    goal: await page.locator("[data-testid='trial-goal']").inputValue(),
    profile: await page.locator("[data-testid='trial-profile']").inputValue(),
    provider,
    model: await page.locator("[data-testid='trial-executor-model']").inputValue(),
    planner_provider: provider,
    planner_model: await page.locator("[data-testid='trial-planner-model']").inputValue(),
  };

  await tokenInput.fill(`${trialToken}-wrong`);
  await checkContract.click();
  await error.waitFor();
  const tokenGuidance = await error.innerText();
  requireIncludes(tokenGuidance, "トークン", "wrong-token guidance");

  let replaceOrigin = true;
  await page.route("**/api/session-proposals", async (route) => {
    if (!replaceOrigin) {
      await route.continue();
      return;
    }
    replaceOrigin = false;
    const response = await route.fetch({
      headers: { ...route.request().headers(), origin: "https://attacker.invalid" },
    });
    await route.fulfill({ response });
  });
  await tokenInput.fill(trialToken);
  await checkContract.click();
  await error.waitFor();
  const originGuidance = await error.innerText();
  requireIncludes(
    originGuidance,
    "GUI_TRIAL_ALLOWED_ORIGINS",
    "foreign-Origin guidance",
  );
  await page.unroute("**/api/session-proposals");

  const proposalResponsePromise = page.waitForResponse((response) =>
    response.url().endsWith("/api/session-proposals"),
  );
  await checkContract.click();
  const proposalResponse = await proposalResponsePromise;
  if (!proposalResponse.ok()) {
    throw new Error(
      `fixture proposal failed with ${proposalResponse.status()}: ${await proposalResponse.text()}`,
    );
  }
  await page.locator("[data-testid='gate-one-card']").waitFor();
  const confirmationHash = await page.locator(".hash-line").innerText();
  const created = await page.evaluate(
    async ({ apiUrl, body, token }) => {
      const response = await fetch(apiUrl, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-commandagent-trial-authorization": `Bearer ${token}`,
        },
        body: JSON.stringify(body),
      });
      const text = await response.text();
      let responseBody;
      try {
        responseBody = JSON.parse(text);
      } catch {
        responseBody = { error: text };
      }
      return { body: responseBody, status: response.status };
    },
    {
      apiUrl: new URL("/api/sessions", server.origin).href,
      body: { ...spec, confirmation_hash: confirmationHash },
      token: trialToken,
    },
  );
  if (created.status !== 202 || typeof created.body.id !== "string") {
    throw new Error(`fixture session was not created: ${JSON.stringify(created)}`);
  }

  await page.locator("[data-testid='gate-one-confirm']").check();
  await page.locator("[data-testid='launch-session']").click();
  const stageError = page.locator(".trial-stage-error[role='alert']");
  await stageError.waitFor();
  const conflictGuidance = await stageError.innerText();
  requireIncludes(conflictGuidance, created.body.id, "running-session ID guidance");
  requireIncludes(conflictGuidance, "再接続", "running-session reconnect guidance");
  const reconnect = page.locator("[data-testid='reconnect-session-link']");
  await reconnect.waitFor();
  requireIncludes(await reconnect.innerText(), created.body.id, "reconnect link session ID");
  await reconnect.click();
  await page.locator("[data-testid='session-progress']").waitFor();
  await page.close();

  const recoveryPage = await browser.newPage();
  await recoveryPage.goto(new URL("/try/", server.origin).href, { waitUntil: "networkidle" });
  await fillTrialContract(recoveryPage);
  const recoveryLeaseRoute = (route) => json(route, 200, { status: "idle" });
  await recoveryPage.route("**/api/trial-workspace", recoveryLeaseRoute);
  await recoveryPage.route("**/api/session-proposals", (route) => json(route, 200, {
    confirmation_required: true,
    card_hash: "sha256:synthetic-recovery-card",
    card_markdown: "# Synthetic recovery contract",
    identity: syntheticIdentity(),
    price: {
      duration_n: 0,
      average_duration_seconds: null,
      cost_n: 0,
      average_cost_usd: null,
      source: "synthetic error smoke",
    },
  }));
  await recoveryPage.locator("[data-testid='check-contract']").click();
  await recoveryPage.locator("[data-testid='gate-one-card']").waitFor();

  const recoveryRoute = async (route) => {
    if (route.request().method() !== "POST") {
      await route.continue();
      return;
    }
    await json(route, 409, {
      code: "trial_workspace_recovery_required",
      error: "synthetic workspace recovery is required",
      session_id: created.body.id,
    });
  };
  await recoveryPage.route("**/api/sessions", recoveryRoute);
  await recoveryPage.locator("[data-testid='gate-one-confirm']").check();
  await recoveryPage.locator("[data-testid='launch-session']").click();
  const recoveryError = recoveryPage.locator(".trial-stage-error[role='alert']");
  await recoveryError.waitFor();
  const recoveryGuidance = await recoveryError.innerText();
  requireIncludes(recoveryGuidance, "復旧", "recovery-required guidance");
  const recoveryLinks = recoveryPage.locator("[data-testid='reconnect-session-link']");
  requireEqual(await recoveryLinks.count(), 1, "recovery reconnect link count");
  requireIncludes(
    await recoveryLinks.innerText(),
    created.body.id,
    "structured recovery session ID",
  );
  await recoveryPage.unroute("**/api/sessions", recoveryRoute);
  await recoveryPage.unroute("**/api/trial-workspace", recoveryLeaseRoute);

  const recoveryRequests = [];
  const recordRecoveryRequest = (request) => {
    if (new URL(request.url()).pathname.endsWith(`/api/sessions/${created.body.id}`)) {
      recoveryRequests.push(request.method());
    }
  };
  await recoveryPage.route(
    `**/api/sessions/${created.body.id}`,
    (route) => json(route, 200, liveSession(created.body.id)),
  );
  recoveryPage.on("request", recordRecoveryRequest);
  await recoveryLinks.click();
  await recoveryPage.locator("[data-testid='session-progress']").waitFor();
  recoveryPage.off("request", recordRecoveryRequest);
  if (
    recoveryRequests.length === 0 ||
    !recoveryRequests.every((method) => method === "GET")
  ) {
    throw new Error(`recovery reconnect was not GET-only: ${JSON.stringify(recoveryRequests)}`);
  }
  await recoveryPage.close();

  const errorPage = await browser.newPage();
  await errorPage.goto(new URL("/try/", server.origin).href, { waitUntil: "networkidle" });
  await errorPage.locator("[data-testid='trial-token']").fill(trialToken);
  const reconnectInput = errorPage.locator("[data-testid='reconnect-session']");
  const reconnectButton = errorPage.locator("[data-testid='reconnect-session-button']");
  const reconnectError = errorPage.locator(".trial-compose > .trial-error[role='alert']");
  const missingSessionId = "00000000-0000-4000-8000-000000000159";
  const missingRequests = [];
  const recordMissingRequest = (request) => {
    if (new URL(request.url()).pathname.endsWith(`/api/sessions/${missingSessionId}`)) {
      missingRequests.push(request.method());
    }
  };
  errorPage.on("request", recordMissingRequest);
  await reconnectInput.fill(missingSessionId);
  await reconnectButton.click();
  await reconnectError.waitFor();
  const missingGuidance = await reconnectError.innerText();
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 1_200));
  errorPage.off("request", recordMissingRequest);
  requireIncludes(missingGuidance, "HTTP 404", "missing-session status guidance");
  requireIncludes(missingGuidance, "セッション ID", "missing-session ID guidance");
  requireEqual(countOccurrences(missingGuidance, "404"), 1, "missing-session 404 message count");
  requireEqual(missingRequests.length, 1, "missing-session reconnect request count");
  requireEqual(missingRequests[0], "GET", "missing-session reconnect method");
  if (missingGuidance.includes("バックオフ")) {
    throw new Error(`missing-session guidance incorrectly promises retry: ${missingGuidance}`);
  }

  const pollingMissingId = "00000000-0000-4000-8000-000000000404";
  let pollingMissingRequests = 0;
  const pollingMissingRoute = async (route) => {
    pollingMissingRequests += 1;
    if (pollingMissingRequests === 1) {
      await json(route, 200, liveSession(pollingMissingId));
      return;
    }
    await json(route, 404, {
      code: "trial_session_not_found",
      error: "synthetic session does not exist",
    });
  };
  await errorPage.route(`**/api/sessions/${pollingMissingId}`, pollingMissingRoute);
  await reconnectInput.fill(pollingMissingId);
  await reconnectButton.click();
  const monitor = errorPage.locator("[data-testid='monitor-state']");
  await monitor.waitFor();
  await waitForMonitorLoss(errorPage);
  const pollingMissingGuidance = await monitor.innerText();
  requireIncludes(pollingMissingGuidance, "再接続", "polling 404 reconnect action");
  requireIncludes(pollingMissingGuidance, "新しい実行", "polling 404 new-run action");
  requireIncludes(pollingMissingGuidance, "4 回失敗", "polling 404 failure bound");
  requireEqual(pollingMissingRequests, 5, "polling 404 request bound");
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 1_200));
  requireEqual(pollingMissingRequests, 5, "polling 404 stopped request count");
  await errorPage.close();

  const invalidPage = await browser.newPage();
  await invalidPage.goto(new URL("/try/", server.origin).href, { waitUntil: "networkidle" });
  await invalidPage.locator("[data-testid='trial-token']").fill(trialToken);
  const invalidReconnectInput = invalidPage.locator("[data-testid='reconnect-session']");
  const invalidReconnectButton = invalidPage.locator("[data-testid='reconnect-session-button']");
  const invalidSessionId = "00000000-0000-4000-8000-000000000170";
  let invalidEventRequests = 0;
  const invalidEventRoute = async (route) => {
    invalidEventRequests += 1;
    if (invalidEventRequests === 1) {
      await json(route, 200, liveSession(invalidSessionId));
      return;
    }
    await json(route, 500, {
      code: "trial_session_events_invalid",
      error: "synthetic malformed event stream",
    });
  };
  await invalidPage.route(`**/api/sessions/${invalidSessionId}`, invalidEventRoute);
  await invalidReconnectInput.fill(invalidSessionId);
  await invalidReconnectButton.click();
  const invalidMonitor = invalidPage.locator("[data-testid='monitor-state']");
  await invalidMonitor.waitFor();
  await waitForMonitorLoss(invalidPage);
  const invalidEventGuidance = await invalidMonitor.innerText();
  requireIncludes(invalidEventGuidance, "イベント JSONL", "invalid-event guidance");
  requireIncludes(invalidEventGuidance, "修復", "invalid-event repair action");
  requireIncludes(invalidEventGuidance, "4 回失敗", "invalid-event failure bound");
  requireEqual(invalidEventRequests, 5, "invalid-event request bound");
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 1_200));
  requireEqual(invalidEventRequests, 5, "invalid-event stopped request count");
  await invalidPage.close();

  console.log(
    JSON.stringify(
      {
        origin_guidance: originGuidance,
        polling_404: {
          guidance: pollingMissingGuidance,
          requests: pollingMissingRequests,
        },
        recovery_required: {
          guidance: recoveryGuidance,
          reconnect_methods: recoveryRequests,
        },
        running_session: created.body.id,
        invalid_events: {
          guidance: invalidEventGuidance,
          requests: invalidEventRequests,
        },
        missing_session: {
          guidance: missingGuidance,
          requests: missingRequests,
        },
        token_guidance: tokenGuidance,
        ok: true,
      },
      null,
      2,
    ),
  );
} finally {
  await browser?.close();
  server?.stop();
  await rm(scratchRoot, { recursive: true, force: true });
}

async function startServer(workspace, commandagentBin) {
  const child = spawn(
    "cargo",
    [
      "run",
      "--features",
      "gui",
      "--bin",
      "gui_server",
      "--",
      "--port",
      "0",
      "--base-path",
      "/",
      "--static-dir",
      join(guiRoot, "out"),
      "--repository-root",
      repositoryRoot,
      "--execution-root",
      workspace,
      "--trial-token-auth",
      "on",
      "--commandagent-bin",
      commandagentBin,
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, GUI_TRIAL_TOKEN: trialToken },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  let diagnostics = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    diagnostics = `${diagnostics}${chunk}`.slice(-12_000);
  });
  child.stdout.setEncoding("utf8");
  const origin = await new Promise((resolveOrigin, reject) => {
    const timer = setTimeout(() => {
      child.kill("SIGTERM");
      reject(new Error(`GUI server start timed out\n${diagnostics}`));
    }, 60_000);
    child.stdout.on("data", (chunk) => {
      const match = chunk.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (match !== null) {
        clearTimeout(timer);
        resolveOrigin(match[1]);
      }
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`GUI server exited with ${code}\n${diagnostics}`));
    });
  });
  return { origin, stop: () => child.kill("SIGTERM") };
}

async function runChecked(command, arguments_, cwd, env) {
  await new Promise((resolveRun, reject) => {
    const child = spawn(command, arguments_, { cwd, env, stdio: "inherit" });
    child.once("exit", (code) => {
      if (code === 0) resolveRun();
      else reject(new Error(`${command} exited with ${code}`));
    });
  });
}

async function json(route, status, body) {
  await route.fulfill({ contentType: "application/json", status, body: JSON.stringify(body) });
}

async function fillTrialContract(page) {
  await page.locator("[data-testid='trial-goal']").fill("Exercise recovery-required handling");
  await page.locator("[data-testid='trial-executor-model']").fill("fixture-executor");
  await page.locator("[data-testid='trial-planner-model']").fill("fixture-planner");
  await page.locator("[data-testid='trial-token']").fill(trialToken);
}

function liveSession(id) {
  return {
    id,
    started_epoch_seconds: 1_723_769_600,
    average_duration_seconds: null,
    gate: "gate_2",
    status: "running",
    verdict: null,
    assurance: null,
    phases: [],
    event_count: 0,
    acceptance_sheet: null,
    section5: null,
    events_path: `.commandagent/runs/${id}/events.jsonl`,
    identity: syntheticIdentity(),
  };
}

function syntheticIdentity() {
  return {
    request: "Exercise bounded monitor failures",
    workspace: "<execution-root>",
    profile: "python-cli",
    intent: "create",
    task_family: "cli",
    route_bases: [],
    contract_ref: "synthetic error smoke",
    contract_checks: [],
    band_full: 0,
    band_denominator: 0,
    band_rate: "not measured",
    band_arm: "smoke",
    band_measurement: "synthetic monitor response",
    band_source: "gui/scripts/error-smoke.mjs",
    full_meaning: "This monitor probe does not delegate a CLI process.",
    pins: {
      planner_provider: "ollama",
      planner_model: "fixture-planner",
      executor_provider: "ollama",
      executor_model: "fixture-executor",
      preset: "profile",
    },
    pack: { selection: "none" },
  };
}

async function waitForMonitorLoss(page) {
  await page.waitForFunction(
    () => document.querySelector("[data-testid='monitor-state']")?.getAttribute(
      "data-monitor-status",
    ) === "lost",
    null,
    { timeout: 15_000 },
  );
}

function requireIncludes(value, expected, label) {
  if (!value.includes(expected)) {
    throw new Error(`${label} did not include ${JSON.stringify(expected)}: ${value}`);
  }
}

function requireEqual(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(`${label} expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

function countOccurrences(value, expected) {
  return value.split(expected).length - 1;
}
