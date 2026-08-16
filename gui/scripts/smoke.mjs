import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const guiRoot = resolve(scriptDirectory, "..");
const repositoryRoot = resolve(guiRoot, "..");
const arguments_ = process.argv.slice(2);
const outputDirectory = valueArgument(arguments_, "--output");
const commandagentBin = resolve(
  valueArgument(arguments_, "--commandagent-bin") ?? join(repositoryRoot, "target/release/commandagent"),
);
const fixtureRoot = resolve(
  valueArgument(arguments_, "--fixture") ??
    join(repositoryRoot, "tests/corpus/apps/test0725_cli_elev_003/fixtures"),
);
const model = valueArgument(arguments_, "--model") ?? "qwen3:8b";
const trialCredential = process.env.GUI_TRIAL_TOKEN ?? randomBytes(32).toString("hex");
const trialTimeoutMs = Number(valueArgument(arguments_, "--trial-timeout-ms") ?? 1_800_000);
const managedPlaywrightPath =
  process.env.COMMANDAGENT_PLAYWRIGHT_PATH ??
  join(homedir(), ".anvil", "tools", "interaction-probe", "node_modules", "playwright");

if (outputDirectory === null) {
  console.error(
    "usage: npm run smoke -- --output <evidence-directory> [--commandagent-bin <path>] [--model <name>]",
  );
  process.exit(2);
}
if (!Number.isFinite(trialTimeoutMs) || trialTimeoutMs <= 0) {
  console.error("--trial-timeout-ms must be a positive number");
  process.exit(2);
}

await mkdir(outputDirectory, { recursive: true });
const packageMetadata = JSON.parse(
  await readFile(join(managedPlaywrightPath, "package.json"), "utf8"),
);
const require = createRequire(import.meta.url);
const { chromium } = require(managedPlaywrightPath);
const scratchRoot = await mkdtemp(join(tmpdir(), "commandagent-g1-gui-smoke-"));

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
    try {
      results.push(await runCase(smokeCase));
    } catch (reason) {
      results.push({
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        error: message(reason),
        ok: false,
      });
      break;
    }
  }
} finally {
  if (results.length === cases.length && results.every((result) => result.ok)) {
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 1_000));
    await rm(scratchRoot, { recursive: true, force: true });
  }
}

const report = {
  schema_version: "commandagent.gui-smoke/v2",
  generated_at: new Date().toISOString(),
  playwright: {
    source: "managed_interaction_probe",
    version: packageMetadata.version,
  },
  delegate: {
    commandagent_bin: commandagentBin,
    provider: "ollama",
    model,
    fixture: fixtureRoot,
    scratch_runtime:
      results.length === cases.length && results.every((result) => result.ok)
        ? "removed_after_success"
        : scratchRoot,
  },
  cases: results,
  ok: results.every((result) => result.ok),
};
await writeFile(join(outputDirectory, "browser-smoke.json"), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
if (!report.ok) process.exitCode = 1;

async function runCase(smokeCase) {
  const startedAt = Date.now();
  const executionRoot = join(scratchRoot, smokeCase.id);
  await cp(fixtureRoot, executionRoot, { recursive: true });
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const server = await startServer(smokeCase.serverBasePath, executionRoot);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1050 } });
  const consoleErrors = [];
  const apiCalls = [];
  page.on("console", (entry) => {
    if (entry.type() === "error") consoleErrors.push(entry.text());
  });
  page.on("response", (response) => {
    const url = new URL(response.url());
    if (url.pathname.includes("/api/session")) {
      apiCalls.push({
        method: response.request().method(),
        path: `${url.pathname}${url.search}`,
        status: response.status(),
      });
    }
  });

  try {
    const prefix = displayBasePath(smokeCase.serverBasePath);
    const dashboardUrl = new URL(prefix, server.origin).href;
    const response = await page.goto(dashboardUrl, { waitUntil: "networkidle" });
    await page.locator("[data-testid='score-time-map']").waitFor();
    const heading = await page.locator("h1").innerText();
    const map = await page.locator("[data-testid='score-time-map']").evaluate((image) => ({
      complete: image.complete,
      naturalWidth: image.naturalWidth,
      source: image.getAttribute("src"),
    }));
    const apiChecks = await page.evaluate(async () => {
      const endpoints = ["runs", "bands", "maps", "packs", "contracts", "suites", "reports"];
      const apiPrefix = document.querySelector("[data-testid='score-time-map']")
        ?.getAttribute("src")
        ?.replace(/maps\/score-time\.svg$/, "");
      return Promise.all(
        endpoints.map(async (endpoint) => {
          const result = await fetch(`${apiPrefix}${endpoint}`);
          return { endpoint, status: result.status, contentType: result.headers.get("content-type") };
        }),
      );
    });
    const internalLinks = await page.locator("a[href]").evaluateAll((links) =>
      links.map((link) => link.getAttribute("href") ?? ""),
    );
    const expectedPrefix = smokeCase.serverBasePath === "/" ? "/" : `${smokeCase.serverBasePath}/`;
    const linksUseBasePath = internalLinks.every((link) => link.startsWith(expectedPrefix));
    const firstRunId = await page.evaluate(async () => {
      const mapSource =
        document.querySelector("[data-testid='score-time-map']")?.getAttribute("src") ?? "";
      const apiRoot = mapSource.replace(/maps\/score-time\.svg$/, "");
      const result = await fetch(`${apiRoot}runs`);
      const runs = await result.json();
      return runs[0]?.id ?? "";
    });
    const runLedgerAccessibility = await page.locator(".run-table").evaluate((ledger) => ({
      columnHeadingsHidden:
        ledger.querySelector(".run-table-head")?.getAttribute("aria-hidden") === "true",
      invalidTableRoleCount:
        (ledger.matches('[role="table"], [role="row"]') ? 1 : 0) +
        ledger.querySelectorAll('[role="table"], [role="row"]').length,
      nativeLinkRows: [...ledger.querySelectorAll(".run-row")].every(
        (row) => row.tagName === "A" && !row.hasAttribute("role"),
      ),
    }));

    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-dashboard.png`),
    });
    const assets = await probePage(
      page,
      server.origin,
      smokeCase.serverBasePath,
      "assets/",
      "Pinned means visible.",
    );
    const measurements = await probePage(
      page,
      server.origin,
      smokeCase.serverBasePath,
      "measurements/",
      "Claims need coordinates.",
    );
    const runDetail = await probePage(
      page,
      server.origin,
      smokeCase.serverBasePath,
      `runs/?id=${encodeURIComponent(firstRunId)}`,
      "One run. Every receipt.",
    );
    await page.locator(".document-viewer").waitFor();

    const trialUrl = new URL(`${prefix}try/`, server.origin).href;
    const trialResponse = await page.goto(trialUrl, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-goal']").fill("Create a CLI --pattern filter command");
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    const modelInputs = page.locator(".trial-fields input");
    await modelInputs.nth(0).fill(model);
    await modelInputs.nth(1).fill(model);
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    const launch = page.locator("[data-testid='launch-session']");
    const launchDisabledBeforeConfirmation = await launch.isDisabled();
    const gateOneText = await page.locator("[data-testid='gate-one-card']").innerText();
    const desktopTrialAlignment = await trialControlAlignment(page);
    const deniedWithoutConfirmation = await page.evaluate(
      async ({ apiUrl, modelName, trialToken }) => {
        const result = await fetch(apiUrl, {
          method: "POST",
          headers: {
            authorization: `Bearer ${trialToken}`,
            "content-type": "application/json",
          },
          body: JSON.stringify({
            goal: "Create a CLI --pattern filter command",
            profile: "python-cli",
            provider: "ollama",
            model: modelName,
            planner_provider: "ollama",
            planner_model: modelName,
          }),
        });
        return { status: result.status, body: await result.json() };
      },
      {
        apiUrl: new URL(`${prefix}api/sessions`, server.origin).href,
        modelName: model,
        trialToken: trialCredential,
      },
    );
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-1.png`),
    });
    await page.setViewportSize({ width: 390, height: 844 });
    const mobileTrialAlignment = await trialControlAlignment(page);
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-1-mobile.png`),
    });

    const injectedFailureMode = smokeCase.id === "proxy-commandagent" ? "access" : "network";
    await installPollFailure(page, injectedFailureMode);
    await page.locator("[data-testid='gate-one-confirm']").check();
    await launch.click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const sessionId = await page.locator("[data-testid='session-progress'] h2").innerText();
    const degradedMonitor = page.locator("[data-testid='monitor-state'][data-monitor-status='degraded']");
    await degradedMonitor.waitFor();
    const degradedMonitorText = await degradedMonitor.innerText();
    const injectedFailureCount = await page.evaluate(
      () => window.__commandagentTrialPollInjection?.count ?? 0,
    );
    const executionScroll = await mobileStageScroll(page, "[data-testid='session-progress']");
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-2-mobile.png`),
    });
    await page.setViewportSize({ width: 1440, height: 1050 });
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-2.png`),
    });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator("[data-testid='terminal-gate']").waitFor({ timeout: trialTimeoutMs });
    const terminalScroll = await mobileStageScroll(page, "[data-testid='terminal-gate']");
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-terminal-mobile.png`),
    });
    const finalApi = await page.evaluate(
      async ({ apiUrl, trialToken }) => {
        const result = await fetch(apiUrl, {
          headers: { authorization: `Bearer ${trialToken}` },
        });
        return { status: result.status, body: await result.json() };
      },
      {
        apiUrl: new URL(`${prefix}api/sessions/${encodeURIComponent(sessionId)}`, server.origin).href,
        trialToken: trialCredential,
      },
    );
    const terminalText = await page.locator("[data-testid='terminal-gate']").innerText();
    const connectedMonitorText = await page.locator("[data-testid='monitor-state']").innerText();
    await page.setViewportSize({ width: 1440, height: 1050 });
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-terminal.png`),
    });

    const reconnectCallStart = apiCalls.length;
    await page.reload({ waitUntil: "networkidle" });
    const reloadSessionQuery = new URL(page.url()).searchParams.get("session");
    const reloadedTokenEmpty = (await page.locator("[data-testid='trial-token']").inputValue()) === "";
    await page.locator("[data-testid='trial-token']").fill(`${trialCredential}-wrong`);
    await page.locator("[data-testid='reconnect-session-button']").click();
    const authorizationGuidance = await page.locator(".trial-error[role='alert']").innerText();
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await page.locator("[data-testid='reconnect-session-button']").click();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const reconnectCalls = apiCalls.slice(reconnectCallStart);
    const reconnectMethods = reconnectCalls.map((call) => call.method);
    const reconnectOnlyGets =
      reconnectMethods.length >= 2 && reconnectMethods.every((method) => method === "GET");
    const browserStorage = await page.evaluate(() => ({
      localStorageValues: Object.values(localStorage),
      url: window.location.href,
    }));
    const tokenStayedInMemory =
      !browserStorage.url.includes(trialCredential) &&
      browserStorage.localStorageValues.every((value) => !value.includes(trialCredential));

    await page.goto(trialUrl, { waitUntil: "networkidle" });
    await installSessionConflict(page, sessionId);
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await modelInputs.nth(0).fill(model);
    await modelInputs.nth(1).fill(model);
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    const conflictGuidance = await page.locator(".trial-error[role='alert']").innerText();
    const conflictReconnectId = await page.locator("[data-testid='reconnect-session']").inputValue();
    const conflictSessionQuery = new URL(page.url()).searchParams.get("session");
    const conflictDispatchCount = await page.evaluate(
      () => window.__commandagentTrialConflictInjection?.count ?? 0,
    );

    const mobile = await probeMobile(browser, server.origin, smokeCase.serverBasePath);

    const eventsPath = join(executionRoot, ".anvil", "runs", sessionId, "events.jsonl");
    const eventBytes = await readFile(eventsPath);
    await writeFile(join(outputDirectory, `${smokeCase.id}-events.jsonl`), eventBytes);
    const apiLog = {
      schema_version: "commandagent.gui-api-smoke/v1",
      base_path: smokeCase.buildBasePath,
      denied_without_confirmation: deniedWithoutConfirmation,
      observed_calls: apiCalls,
      poll_recovery: {
        failure_mode: injectedFailureMode,
        injected_failure_count: injectedFailureCount,
        degraded_text: degradedMonitorText,
        connected_text: connectedMonitorText,
      },
      reconnect: {
        authorization_guidance: authorizationGuidance,
        calls: reconnectCalls,
        only_gets: reconnectOnlyGets,
      },
      terminal_poll: finalApi,
    };
    await writeFile(
      join(outputDirectory, `${smokeCase.id}-api-log.json`),
      `${JSON.stringify(apiLog, null, 2)}\n`,
    );
    const expectedNegativeConsoleErrors = consoleErrors.filter(
      (entry) =>
        entry === "Failed to load resource: the server responded with a status of 428 (Precondition Required)" ||
        entry === "Failed to load resource: the server responded with a status of 401 (Unauthorized)",
    );
    const unexpectedConsoleErrors = consoleErrors.filter(
      (entry) => !expectedNegativeConsoleErrors.includes(entry),
    );

    const ok =
      response?.status() === 200 &&
      heading === "Evidence, at a glance." &&
      map.complete &&
      map.naturalWidth > 0 &&
      apiChecks.every((check) => check.status === 200) &&
      linksUseBasePath &&
      runLedgerAccessibility.columnHeadingsHidden &&
      runLedgerAccessibility.invalidTableRoleCount === 0 &&
      runLedgerAccessibility.nativeLinkRows &&
      assets.status === 200 &&
      assets.headingMatches &&
      measurements.status === 200 &&
      measurements.headingMatches &&
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      trialResponse?.status() === 200 &&
      desktopTrialAlignment.aligned &&
      mobileTrialAlignment.aligned &&
      launchDisabledBeforeConfirmation &&
      deniedWithoutConfirmation.status === 428 &&
      executionScroll.clearsStickyHeader &&
      terminalScroll.clearsStickyHeader &&
      finalApi.status === 200 &&
      ["gate_3", "gate_4"].includes(finalApi.body.gate) &&
      injectedFailureCount === 1 &&
      degradedMonitorText.includes(
        injectedFailureMode === "access" ? "upstream access" : "proxy or network",
      ) &&
      connectedMonitorText.toLowerCase().includes("monitoring: connected") &&
      connectedMonitorText.includes("Last successful update:") &&
      reloadSessionQuery === sessionId &&
      reloadedTokenEmpty &&
      authorizationGuidance.includes("runtime Trial access token") &&
      reconnectOnlyGets &&
      tokenStayedInMemory &&
      conflictGuidance.includes(`Reconnect to session ${sessionId}`) &&
      conflictReconnectId === sessionId &&
      conflictSessionQuery === sessionId &&
      conflictDispatchCount === 1 &&
      mobile.ok &&
      expectedNegativeConsoleErrors.some((entry) => entry.includes("status of 428")) &&
      expectedNegativeConsoleErrors.some((entry) => entry.includes("status of 401")) &&
      unexpectedConsoleErrors.length === 0;
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      dashboard: { status: response?.status() ?? 0, heading },
      api_checks: apiChecks,
      svg: map,
      links_use_base_path: linksUseBasePath,
      run_ledger_accessibility: runLedgerAccessibility,
      pages: { assets, measurements, run_detail: runDetail, trial: { status: trialResponse?.status() ?? 0 } },
      mobile,
      gate_1: {
        launch_disabled_before_confirmation: launchDisabledBeforeConfirmation,
        api_without_confirmation_status: deniedWithoutConfirmation.status,
        control_alignment: {
          desktop_1440: desktopTrialAlignment,
          mobile_390: mobileTrialAlignment,
        },
        visible_text: gateOneText,
      },
      session: {
        id: sessionId,
        gate: finalApi.body.gate,
        status: finalApi.body.status,
        verdict: finalApi.body.verdict,
        assurance: finalApi.body.assurance,
        event_count: finalApi.body.event_count,
        events_sha256: `sha256:${createHash("sha256").update(eventBytes).digest("hex")}`,
        mobile_stage_scroll: {
          execution: executionScroll,
          terminal: terminalScroll,
        },
        terminal_visible_text: terminalText,
      },
      monitoring: {
        failure_mode: injectedFailureMode,
        injected_failure_count: injectedFailureCount,
        degraded_visible_text: degradedMonitorText,
        connected_visible_text: connectedMonitorText,
      },
      reconnect: {
        reload_session_query: reloadSessionQuery,
        token_empty_after_reload: reloadedTokenEmpty,
        authorization_guidance: authorizationGuidance,
        calls: reconnectCalls,
        only_gets: reconnectOnlyGets,
        token_stayed_in_memory: tokenStayedInMemory,
      },
      conflict_reconnect: {
        guidance: conflictGuidance,
        reconnect_id: conflictReconnectId,
        session_query: conflictSessionQuery,
        intercepted_dispatches: conflictDispatchCount,
      },
      elapsed_seconds: (Date.now() - startedAt) / 1000,
      expected_negative_console_errors: expectedNegativeConsoleErrors,
      unexpected_console_errors: unexpectedConsoleErrors,
      ok,
    };
  } finally {
    await browser.close();
    server.stop();
  }
}

async function probeMobile(browser, origin, basePath) {
  const page = await browser.newPage({
    isMobile: true,
    viewport: { width: 390, height: 844 },
  });
  try {
    const prefix = displayBasePath(basePath);
    const dashboard = await page.goto(new URL(prefix, origin).href, { waitUntil: "networkidle" });
    const dashboardHeading = await page.locator("h1").innerText();
    const dashboardFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    const trial = await page.goto(new URL(`${prefix}try/`, origin).href, {
      waitUntil: "networkidle",
    });
    const trialHeading = await page.locator("h1").innerText();
    await page.locator("[data-testid='reconnect-card']").waitFor();
    const trialFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    return {
      dashboard: { fits_viewport: dashboardFits, heading: dashboardHeading, status: dashboard?.status() ?? 0 },
      trial: { fits_viewport: trialFits, heading: trialHeading, status: trial?.status() ?? 0 },
      ok:
        dashboard?.status() === 200 &&
        dashboardHeading === "Evidence, at a glance." &&
        dashboardFits &&
        trial?.status() === 200 &&
        trialHeading === "Launch once. Trust the gates." &&
        trialFits,
    };
  } finally {
    await page.close();
  }
}

async function installPollFailure(page, mode) {
  await page.evaluate((failureMode) => {
    const nativeFetch = window.fetch.bind(window);
    window.__commandagentTrialPollInjection = { count: 0, mode: failureMode };
    window.fetch = async (input, init) => {
      const request = input instanceof Request ? input : null;
      const method = (init?.method ?? request?.method ?? "GET").toUpperCase();
      const rawUrl = typeof input === "string" ? input : request?.url ?? String(input);
      const path = new URL(rawUrl, window.location.href).pathname;
      const injection = window.__commandagentTrialPollInjection;
      if (
        injection !== undefined &&
        injection.count === 0 &&
        method === "GET" &&
        /\/api\/sessions\/[0-9a-f-]{36}$/.test(path)
      ) {
        injection.count += 1;
        if (failureMode === "network") {
          throw new TypeError("Synthetic browser fetch rejection");
        }
        const response = new Response(null, { status: 200 });
        return new Proxy(response, {
          get(target, property) {
            if (property === "type") return "opaqueredirect";
            const value = Reflect.get(target, property, target);
            return typeof value === "function" ? value.bind(target) : value;
          },
        });
      }
      return nativeFetch(input, init);
    };
  }, mode);
}

async function installSessionConflict(page, sessionId) {
  await page.evaluate((activeSessionId) => {
    const nativeFetch = window.fetch.bind(window);
    window.__commandagentTrialConflictInjection = { count: 0 };
    window.fetch = async (input, init) => {
      const request = input instanceof Request ? input : null;
      const method = (init?.method ?? request?.method ?? "GET").toUpperCase();
      const rawUrl = typeof input === "string" ? input : request?.url ?? String(input);
      const path = new URL(rawUrl, window.location.href).pathname;
      if (method === "POST" && /\/api\/sessions$/.test(path)) {
        window.__commandagentTrialConflictInjection.count += 1;
        return new Response(
          JSON.stringify({ error: `trial workspace is already running session ${activeSessionId}` }),
          { headers: { "content-type": "application/json" }, status: 409 },
        );
      }
      return nativeFetch(input, init);
    };
  }, sessionId);
}

async function trialControlAlignment(page) {
  const [goal, token] = await Promise.all([
    page.locator("[data-testid='trial-goal']").boundingBox(),
    page.locator("[data-testid='trial-token']").boundingBox(),
  ]);
  if (goal === null || token === null) throw new Error("Trial controls are not visible");
  const leftDelta = Math.abs(goal.x - token.x);
  const rightDelta = Math.abs(goal.x + goal.width - (token.x + token.width));
  return {
    aligned: leftDelta <= 1 && rightDelta <= 1,
    goal: { left: goal.x, right: goal.x + goal.width },
    left_delta_px: leftDelta,
    right_delta_px: rightDelta,
    token: { left: token.x, right: token.x + token.width },
    viewport_width: page.viewportSize()?.width ?? 0,
  };
}

async function mobileStageScroll(page, selector) {
  await page.waitForFunction((targetSelector) => {
    const target = document.querySelector(targetSelector);
    const topbar = document.querySelector(".topbar");
    const heading = target?.querySelector("h2");
    if (target === null || topbar === null || heading === null) return false;
    const headingBounds = heading.getBoundingClientRect();
    const topbarBottom = topbar.getBoundingClientRect().bottom;
    const scrollMarginTop = Number.parseFloat(getComputedStyle(target).scrollMarginTop);
    return (
      scrollMarginTop > topbar.getBoundingClientRect().height &&
      headingBounds.top >= topbarBottom - 1 &&
      headingBounds.bottom <= window.innerHeight
    );
  }, selector);
  return page.locator(selector).evaluate((target) => {
    const topbar = document.querySelector(".topbar");
    const heading = target.querySelector("h2");
    if (topbar === null || heading === null) {
      throw new Error("Stage heading or sticky top bar is missing");
    }
    const targetTop = target.getBoundingClientRect().top;
    const headingBounds = heading.getBoundingClientRect();
    const topbarBottom = topbar.getBoundingClientRect().bottom;
    return {
      clearance_px: headingBounds.top - topbarBottom,
      clearsStickyHeader:
        headingBounds.top >= topbarBottom - 1 && headingBounds.bottom <= window.innerHeight,
      heading_bottom_px: headingBounds.bottom,
      heading_top_px: headingBounds.top,
      scroll_margin_top_px: Number.parseFloat(getComputedStyle(target).scrollMarginTop),
      target_top_px: targetTop,
      topbar_bottom_px: topbarBottom,
    };
  });
}

async function probePage(page, origin, basePath, relativePath, expectedHeading) {
  const prefix = displayBasePath(basePath);
  const url = new URL(`${prefix}${relativePath}`, origin).href;
  const response = await page.goto(url, { waitUntil: "networkidle" });
  const heading = await page.locator("h1").innerText();
  return { status: response?.status() ?? 0, heading, headingMatches: heading === expectedHeading };
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
      "--commandagent-bin",
      commandagentBin,
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, GUI_TRIAL_TOKEN: trialCredential },
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

function displayBasePath(basePath) {
  return basePath === "/" ? "/" : `${basePath}/`;
}

function valueArgument(arguments_, name) {
  const index = arguments_.indexOf(name);
  if (index === -1 || arguments_[index + 1] === undefined) return null;
  return resolveIfPath(name, arguments_[index + 1]);
}

function resolveIfPath(name, value) {
  return ["--output", "--commandagent-bin", "--fixture"].includes(name) ? resolve(value) : value;
}

function message(reason) {
  return reason instanceof Error ? reason.stack ?? reason.message : String(reason);
}
