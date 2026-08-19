import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
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
const outputDirectory = valueArgument(process.argv.slice(2), "--output");
const trialToken = "commandagent-session-index-smoke-token-000000000100";
const createdSessionId = "0198b9c8-fab8-7000-8000-000000000100";
const existingSessionId = "0198b9c8-fab8-7000-8000-000000000101";
const scratchRoot = await mkdtemp(join(tmpdir(), "commandagent-session-index-smoke-"));
const require = createRequire(import.meta.url);
const { chromium } = require(managedPlaywrightPath);
const packageMetadata = JSON.parse(
  await readFile(join(managedPlaywrightPath, "package.json"), "utf8"),
);

const cases = [
  { id: "root", buildBasePath: "/", serverBasePath: "/" },
  {
    id: "proxy-commandagent",
    buildBasePath: "/proxy/commandagent/",
    serverBasePath: "/proxy/commandagent",
  },
];
const results = [];

try {
  for (const smokeCase of cases) {
    await runChecked("npm", ["run", "build"], guiRoot, {
      ...process.env,
      GUI_BASE_PATH: smokeCase.buildBasePath,
    });
    const executionRoot = join(scratchRoot, smokeCase.id);
    await mkdir(executionRoot, { recursive: true });
    const server = await startServer(smokeCase.serverBasePath, executionRoot);
    const browser = await chromium.launch({ headless: true });
    try {
      const lifecycle = await probeLifecycle(browser, server.origin, smokeCase.serverBasePath);
      const sourceMatrix = await probeSourceMatrix(
        browser,
        server.origin,
        smokeCase.serverBasePath,
      );
      results.push({
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        lifecycle,
        source_matrix: sourceMatrix,
        ok: lifecycle.ok && sourceMatrix.ok,
      });
    } finally {
      await browser.close();
      server.stop();
    }
  }
} finally {
  if (results.length === cases.length && results.every((result) => result.ok)) {
    await rm(scratchRoot, { recursive: true, force: true });
  }
}

const report = {
  schema_version: "commandagent.gui-session-index-smoke/v1",
  generated_at: new Date().toISOString(),
  playwright: { source: "managed_interaction_probe", version: packageMetadata.version },
  cases: results,
  ok: results.length === cases.length && results.every((result) => result.ok),
};
if (outputDirectory !== null) {
  await mkdir(outputDirectory, { recursive: true });
  await writeFile(
    join(outputDirectory, "session-index-smoke.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}
console.log(JSON.stringify(report, null, 2));
if (!report.ok) process.exitCode = 1;

async function probeLifecycle(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  let indexSessions = [];
  let indexCalls = 0;
  let runtimeCalls = 0;
  let runtimeSession = null;
  let failNextIndex = false;
  let terminalReleased = false;
  let releaseTerminal;
  const terminalReady = new Promise((resolveTerminal) => {
    releaseTerminal = resolveTerminal;
  });
  const sessionRequests = [];

  try {
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      const method = request.method();
      if (pathname.endsWith("/api/runtime-status") && method === "GET") {
        runtimeCalls += 1;
        await json(route, 200, {
          trial_available: true,
          trial_token_auth_enabled: true,
          session: runtimeSession,
        });
        return;
      }
      if (pathname.endsWith("/api/trial-options") && method === "GET") {
        await json(route, 200, syntheticOptions());
        return;
      }
      if (pathname.endsWith("/api/trial-workspace") && method === "GET") {
        await json(route, 200, { status: "idle" });
        return;
      }
      if (pathname.endsWith("/api/session-proposals") && method === "POST") {
        await json(route, 200, syntheticProposal());
        return;
      }
      if (pathname.endsWith("/api/sessions") && method === "GET") {
        indexCalls += 1;
        if (failNextIndex) {
          failNextIndex = false;
          await json(route, 503, {
            code: "synthetic_index_refresh_failed",
            error: "synthetic session index refresh failed",
          });
          return;
        }
        await json(route, 200, { sessions: indexSessions, lease: { status: "idle" } });
        return;
      }
      if (pathname.endsWith("/api/sessions") && method === "POST") {
        sessionRequests.push({ method, pathname });
        await json(route, 202, {
          id: createdSessionId,
          gate: "gate_2",
          status: "starting",
          events_path: `.anvil/runs/${createdSessionId}/events.jsonl`,
        });
        return;
      }
      if (pathname.endsWith(`/api/sessions/${createdSessionId}`)) {
        sessionRequests.push({ method, pathname });
        if (!terminalReleased) await terminalReady;
        indexSessions = [terminalSummary(createdSessionId)];
        await json(route, 200, terminalSession(createdSessionId));
        return;
      }
      if (pathname.endsWith(`/api/sessions/${createdSessionId}/artifacts`)) {
        await json(route, 200, []);
        return;
      }
      await json(route, 404, { error: `unexpected mocked API: ${method} ${pathname}` });
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    const authPending = await page.locator("[data-testid='trial-session-auth-required']").innerText();
    assertIncludes(authPending, "認証待ち", "unauthenticated Trial history state");
    assert(indexCalls === 0, `unauthenticated page issued ${indexCalls} index requests`);

    await page.locator("[data-testid='trial-token']").fill(trialToken);
    await waitFor(() => indexCalls >= 1, "initial token index refresh");
    await page.locator("[data-testid='trial-session-freshness']").waitFor();
    const initialIndexCalls = indexCalls;

    runtimeSession = { id: createdSessionId, state: "running" };
    await page
      .locator("[data-testid='runtime-status'][data-session-state='running']")
      .waitFor({ timeout: 10_000 });
    await delay(3_500);
    const noPeriodicIndexPolling = indexCalls === initialIndexCalls && runtimeCalls >= 2;
    runtimeSession = null;
    await page
      .locator("[data-testid='runtime-status'][data-session-state='idle']")
      .waitFor({ timeout: 10_000 });
    await waitFor(() => indexCalls > initialIndexCalls, "runtime running-to-idle refresh");
    const runtimeLeaseRefreshCalls = indexCalls;

    await page.locator("[data-testid='trial-goal']").fill("Synthetic live Trial history probe");
    await page.locator("[data-testid='trial-executor-model']").fill("synthetic-model");
    await page.locator("[data-testid='trial-planner-model']").fill("synthetic-model");
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    const beforeLaunch = indexCalls;
    await page.locator("[data-testid='launch-session']").click();
    const launchedRow = page.locator(`#trial-session-${createdSessionId}`);
    await launchedRow.waitFor();
    const startingText = await launchedRow.innerText();
    assertIncludes(startingText, createdSessionId, "optimistic launch row ID");
    assertIncludes(startingText, "GATE_2 / STARTING", "optimistic launch row state");
    await waitFor(() => indexCalls > beforeLaunch, "launch acceptance refresh");

    const beforeTerminal = indexCalls;
    terminalReleased = true;
    releaseTerminal();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await waitFor(() => indexCalls > beforeTerminal, "terminal transition refresh");
    await page.waitForFunction(
      (id) => document.querySelector(`#trial-session-${id}`)?.innerText.includes("COMPLETED"),
      createdSessionId,
    );
    const terminalText = await launchedRow.innerText();
    assertIncludes(terminalText, "GATE_3 / COMPLETED", "terminal history state");

    const historyLink = page.locator("[data-testid='terminal-session-history-link']");
    assert(
      (await historyLink.getAttribute("href")) === `#trial-session-${createdSessionId}`,
      "terminal history link did not target the active Trial row",
    );
    await historyLink.click();
    assert(
      new URL(page.url()).hash === `#trial-session-${createdSessionId}`,
      "terminal history link did not navigate to its row",
    );

    const freshnessBeforeFailure = await page
      .locator("[data-testid='trial-session-freshness']")
      .innerText();
    failNextIndex = true;
    await page.locator("[data-testid='refresh-trial-sessions']").click();
    const refreshError = page.locator(".session-index-error[role='alert']");
    await refreshError.waitFor();
    assertIncludes(await refreshError.innerText(), "最後に取得できた一覧", "stale data guidance");
    assert((await launchedRow.count()) === 1, "refresh failure removed the last successful row");
    const freshnessAfterFailure = await page
      .locator("[data-testid='trial-session-freshness']")
      .innerText();
    assert(
      freshnessAfterFailure === freshnessBeforeFailure,
      "failed refresh changed the last-success freshness value",
    );

    const beforeFocus = indexCalls;
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => indexCalls > beforeFocus, "focus refresh");
    await refreshError.waitFor({ state: "hidden" });
    const beforeVisibility = indexCalls;
    await page.evaluate(() => document.dispatchEvent(new Event("visibilitychange")));
    await waitFor(() => indexCalls > beforeVisibility, "visible-tab refresh");

    const reconnectLink = launchedRow.locator("[data-testid='session-reconnect-link']");
    assert(
      (await reconnectLink.getAttribute("href")) === `?session=${createdSessionId}`,
      "session deep link changed",
    );
    await Promise.all([
      page.waitForNavigation({ waitUntil: "networkidle" }),
      reconnectLink.click(),
    ]);
    await page.waitForFunction(
      (token) => document.querySelector("[data-testid='trial-token']")?.value === token,
      trialToken,
    );
    const requestOffset = sessionRequests.length;
    const beforeReconnect = indexCalls;
    await page.locator("[data-testid='reconnect-session-button']").click();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await waitFor(() => indexCalls > beforeReconnect, "reconnect success refresh");
    const reconnectRequests = sessionRequests.slice(requestOffset);
    const reconnectGetOnly =
      reconnectRequests.length > 0 && reconnectRequests.every((request) => request.method === "GET");

    return {
      initial_index_calls: initialIndexCalls,
      runtime_calls: runtimeCalls,
      runtime_lease_refresh_calls: runtimeLeaseRefreshCalls,
      final_index_calls: indexCalls,
      no_periodic_index_polling: noPeriodicIndexPolling,
      optimistic_starting_text: startingText,
      terminal_text: terminalText,
      refresh_error_retained_last_success: (await launchedRow.count()) === 1,
      focus_revalidated: indexCalls > beforeFocus,
      visibility_revalidated: indexCalls > beforeVisibility,
      reconnect_requests: reconnectRequests,
      reconnect_get_only: reconnectGetOnly,
      ok: noPeriodicIndexPolling && reconnectGetOnly,
    };
  } finally {
    await page.close();
  }
}

async function probeSourceMatrix(browser, origin, basePath) {
  const repositoryRun = {
    id: "repository-run-100",
    modified_epoch_seconds: 1_723_769_600,
    report_path: "workspace/management/runs/repository-run-100/acceptance.md",
    status: "completed",
    status_text: "検証済み",
    state: "pass",
  };
  const scenarios = [
    { id: "repository-only", repositoryRuns: [repositoryRun], trialSessions: [], authenticated: true },
    { id: "trial-only", repositoryRuns: [], trialSessions: [terminalSummary(existingSessionId)], authenticated: true },
    { id: "both", repositoryRuns: [repositoryRun], trialSessions: [terminalSummary(existingSessionId)], authenticated: true },
    { id: "trial-unauthenticated", repositoryRuns: [repositoryRun], trialSessions: [terminalSummary(existingSessionId)], authenticated: false },
  ];
  const results = [];

  for (const scenario of scenarios) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    let indexCalls = 0;
    try {
      await page.route("**/api/**", async (route) => {
        const request = route.request();
        const pathname = new URL(request.url()).pathname;
        if (pathname.endsWith("/api/runtime-status")) {
          await json(route, 200, {
            trial_available: true,
            trial_token_auth_enabled: true,
            session: null,
          });
        } else if (pathname.endsWith("/api/trial-options")) {
          await json(route, 200, syntheticOptions());
        } else if (pathname.endsWith("/api/runs")) {
          await json(route, 200, {
            runs: scenario.repositoryRuns,
            total: scenario.repositoryRuns.length,
          });
        } else if (pathname.endsWith("/api/sessions") && request.method() === "GET") {
          indexCalls += 1;
          await json(route, 200, {
            sessions: scenario.trialSessions,
            lease: { status: "idle" },
          });
        } else {
          await json(route, 404, { error: `unexpected matrix API: ${request.method()} ${pathname}` });
        }
      });

      const prefix = displayBasePath(basePath);
      await page.goto(new URL(`${prefix}runs/`, origin).href, { waitUntil: "networkidle" });
      const repositorySource = await page.locator("[data-testid='repository-run-source']").innerText();
      const repositoryCount = await page.locator("#run-select option:not([value=''])").count();
      const repositoryEmpty = await page.locator(".run-picker").innerText();
      assertIncludes(
        repositorySource.toLowerCase(),
        "workspace/management/runs",
        `${scenario.id} repository source`,
      );

      await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
      const trialSource = await page.locator("[data-testid='trial-session-index'] header").innerText();
      assertIncludes(
        trialSource.toLowerCase(),
        "execution root / .anvil/runs",
        `${scenario.id} Trial source`,
      );
      if (scenario.authenticated) {
        await page.locator("[data-testid='trial-token']").fill(trialToken);
        await waitFor(() => indexCalls > 0, `${scenario.id} authenticated index load`);
      }
      const trialCount = await page.locator(".session-list li").count();
      const trialText = await page.locator("[data-testid='trial-session-index']").innerText();
      const expectedRepositoryCount = scenario.repositoryRuns.length;
      const expectedTrialCount = scenario.authenticated ? scenario.trialSessions.length : 0;
      const stateOk =
        repositoryCount === expectedRepositoryCount &&
        trialCount === expectedTrialCount &&
        (expectedRepositoryCount > 0 || repositoryEmpty.includes("repository 記録なし")) &&
        (scenario.authenticated || (indexCalls === 0 && trialText.includes("認証待ち")));
      results.push({
        id: scenario.id,
        repository_count: repositoryCount,
        trial_count: trialCount,
        trial_index_calls: indexCalls,
        state_ok: stateOk,
      });
    } finally {
      await page.close();
    }
  }

  return { scenarios: results, ok: results.every((result) => result.state_ok) };
}

function syntheticOptions() {
  return {
    profiles: [{
      id: "python-cli",
      label: "Python CLI",
      description: "synthetic",
      status: "admitted",
      manifest_hash: null,
      assurance_ceiling: "full",
      base_profile: null,
    }],
    providers: [{ id: "ollama", label: "Ollama", model_hint: "synthetic-model" }],
  };
}

function syntheticProposal() {
  return {
    confirmation_required: true,
    card_hash: `sha256:${"1".repeat(64)}`,
    card_markdown: "# Synthetic live Trial history contract",
    identity: {
      request: "Synthetic live Trial history probe",
      workspace: "/synthetic/session-index",
      profile: "python-cli",
      intent: "create",
      task_family: "cli",
      route_bases: ["smoke=synthetic"],
      contract_ref: "synthetic/session-index",
      contract_checks: ["event-driven session index"],
      band_full: 1,
      band_denominator: 1,
      band_rate: "1/1",
      band_arm: "smoke",
      band_measurement: "mocked lifecycle",
      band_source: "gui/scripts/session-index-smoke.mjs",
      full_meaning: "The smoke does not delegate a CLI process.",
      pins: {
        planner_provider: "ollama",
        planner_model: "synthetic-model",
        executor_provider: "ollama",
        executor_model: "synthetic-model",
        preset: "profile",
      },
    },
    price: {
      duration_n: 0,
      average_duration_seconds: null,
      cost_n: 0,
      average_cost_usd: null,
      source: "synthetic",
    },
  };
}

function terminalSummary(id) {
  return {
    id,
    started_epoch_seconds: 1_723_769_600,
    modified_epoch_seconds: 1_723_769_660,
    gate: "gate_3",
    status: "completed",
    pack: {
      id: "cli-assist",
      version: "1.0.0",
      hash: "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd",
      source: "admitted",
      source_label: "承認済み",
    },
  };
}

function terminalSession(id) {
  return {
    id,
    gate: "gate_3",
    status: "completed",
    verdict: "pass",
    assurance: "full",
    phases: [],
    event_count: 1,
    acceptance_sheet: "# Synthetic acceptance\n\nPASS",
    section5: "PASS",
    events_path: `.anvil/runs/${id}/events.jsonl`,
  };
}

async function json(route, status, body) {
  await route.fulfill({ contentType: "application/json", status, body: JSON.stringify(body) });
}

async function startServer(basePath, executionRoot) {
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
      basePath,
      "--static-dir",
      join(guiRoot, "out"),
      "--repository-root",
      repositoryRoot,
      "--execution-root",
      executionRoot,
      "--trial-token-auth",
      "on",
      "--commandagent-bin",
      process.execPath,
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

async function waitFor(predicate, label) {
  const deadline = Date.now() + 10_000;
  while (!predicate()) {
    if (Date.now() >= deadline) throw new Error(`${label} timed out`);
    await delay(50);
  }
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function assertIncludes(value, expected, label) {
  assert(value.includes(expected), `${label} did not include ${JSON.stringify(expected)}: ${value}`);
}

function displayBasePath(basePath) {
  return basePath === "/" ? "/" : `${basePath}/`;
}

function valueArgument(arguments_, name) {
  const index = arguments_.indexOf(name);
  return index === -1 ? null : arguments_[index + 1] ?? null;
}
