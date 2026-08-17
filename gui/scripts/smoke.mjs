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
const readOnly = arguments_.includes("--read-only");
const overviewOnly = arguments_.includes("--overview-only");
const outputDirectory = valueArgument(arguments_, "--output");
const feedbackOnly = arguments_.includes("--feedback-only");
const pollingOnly = arguments_.includes("--polling-only");
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
    "usage: npm run smoke -- --output <evidence-directory> [--read-only | --overview-only | --feedback-only | --polling-only] [--commandagent-bin <path>] [--model <name>]",
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
      results.push(
        await (feedbackOnly
          ? runFeedbackCase(smokeCase)
          : pollingOnly
            ? runPollingCase(smokeCase)
            : runCase(smokeCase)),
      );
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
    mode: readOnly
      ? "read_only"
      : overviewOnly
        ? "overview_only"
        : feedbackOnly
          ? "feedback_only"
          : pollingOnly
            ? "polling_only"
            : "full_trial",
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

async function runFeedbackCase(smokeCase) {
  const executionRoot = join(scratchRoot, smokeCase.id);
  await mkdir(executionRoot, { recursive: true });
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const server = await startServer(smokeCase.serverBasePath, executionRoot);
  const browser = await chromium.launch({ headless: true });
  try {
    const feedback = await probeTrialFeedback(browser, server.origin, smokeCase.serverBasePath);
    const sessionIndexLease = await probeSessionIndexLease(
      browser,
      server.origin,
      smokeCase.serverBasePath,
    );
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      trial_feedback: feedback,
      session_index_lease: sessionIndexLease,
      ok: feedback.ok && sessionIndexLease.ok,
    };
  } finally {
    await browser.close();
    server.stop();
  }
}

async function runPollingCase(smokeCase) {
  const executionRoot = join(scratchRoot, smokeCase.id);
  await mkdir(executionRoot, { recursive: true });
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const server = await startServer(smokeCase.serverBasePath, executionRoot);
  const browser = await chromium.launch({ headless: true });
  try {
    const polling = await probeTenMinutePolling(browser, server.origin, smokeCase.serverBasePath);
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      duration_ms: polling.duration_ms,
      fixed_750ms_calls: polling.fixed_750ms_calls,
      observed_call_count: polling.observed_call_count,
      observed_calls: polling.observed_calls,
      conditional_requests: polling.conditional_requests,
      reduction_percent: polling.reduction_percent,
      ok: polling.ok,
    };
  } finally {
    await browser.close();
    server.stop();
  }
}

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
    const dashboardTitle = await page.title();
    await page.locator("[data-testid='runtime-status'][data-trial-available='true'][data-session-state='idle']").waitFor();
    const idleRuntimeText = await page.locator("[data-testid='runtime-status']").innerText();
    const map = await page.locator("[data-testid='score-time-map']").evaluate((image) => ({
      complete: image.complete,
      naturalWidth: image.naturalWidth,
      source: image.getAttribute("src"),
    }));
    const apiChecks = await page.evaluate(async () => {
      const endpoints = [
        "runs",
        "bands",
        "maps",
        "packs",
        "contracts",
        "suites",
        "reports",
        "runtime-status",
        "trial-options",
      ];
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
    const primaryNavigation = await page.locator(".sidebar .nav-link").allInnerTexts();
    const assetsLink = await page.locator("[data-testid='assets-link']").getAttribute("href");
    const runIndex = await page.evaluate(async () => {
      const mapSource =
        document.querySelector("[data-testid='score-time-map']")?.getAttribute("src") ?? "";
      const apiRoot = mapSource.replace(/maps\/score-time\.svg$/, "");
      const result = await fetch(`${apiRoot}runs`);
      return result.json();
    });
    const runCountText = await page.locator("[data-testid='run-count']").innerText();
    const expectedRunCountText = `${Math.min(runIndex.runs.length, 8)} / ${runIndex.total}`;
    const statusBadgeTexts = await page.locator(".status-badge").allInnerTexts();
    const statusBadgesArePlainText = statusBadgeTexts.every(
      (text) => !text.includes("**") && !text.includes("`"),
    );
    const dashboard = {
      assets_link: assetsLink,
      primary_navigation: primaryNavigation,
      status: response?.status() ?? 0,
      heading,
      title: dashboardTitle,
      run_count: runCountText,
      expected_run_count: expectedRunCountText,
      status_badges: statusBadgeTexts,
      status_badges_are_plain_text: statusBadgesArePlainText,
    };
    const dashboardOk =
      response?.status() === 200 &&
      heading === "概要" &&
      dashboardTitle === "概要 | CommandAgent" &&
      map.complete &&
      map.naturalWidth > 0 &&
      apiChecks.every((check) => check.status === 200) &&
      linksUseBasePath &&
      JSON.stringify(primaryNavigation) ===
        JSON.stringify(["01\n概要", "02\nトライアル", "03\n検証・運用レポート", "04\n計測"]) &&
      assetsLink === `${expectedPrefix}assets/` &&
      runCountText === expectedRunCountText &&
      statusBadgesArePlainText;
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
    const dashboardAccessible =
      runLedgerAccessibility.columnHeadingsHidden &&
      runLedgerAccessibility.invalidTableRoleCount === 0 &&
      runLedgerAccessibility.nativeLinkRows;

    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-dashboard.png`),
    });
    if (overviewOnly) {
      return {
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        dashboard,
        api_checks: apiChecks,
        svg: map,
        links_use_base_path: linksUseBasePath,
        run_ledger_accessibility: runLedgerAccessibility,
        elapsed_seconds: (Date.now() - startedAt) / 1000,
        unexpected_console_errors: consoleErrors,
        ok: dashboardOk && dashboardAccessible && consoleErrors.length === 0,
      };
    }
    const assets = await probePage(
      page,
      server.origin,
      smokeCase.serverBasePath,
      "assets/",
      "アセット",
      "アセット | CommandAgent",
    );
    const readOnlyUi = await probeReadOnlyUi(
      page,
      server.origin,
      smokeCase.serverBasePath,
      runIndex.runs,
      smokeCase.id,
    );
    const measurements = readOnlyUi.pages.measurements;
    const runDetail = readOnlyUi.pages.run_detail;
    const readOnlyOk =
      dashboardOk &&
      dashboardAccessible &&
      assets.status === 200 &&
      assets.headingMatches &&
      assets.titleMatches &&
      readOnlyUi.ok &&
      consoleErrors.length === 0;

    if (readOnly) {
      return {
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        dashboard,
        api_checks: apiChecks,
        svg: map,
        links_use_base_path: linksUseBasePath,
        run_ledger_accessibility: runLedgerAccessibility,
        pages: { assets, measurements, run_detail: runDetail },
        issue_75: readOnlyUi,
        elapsed_seconds: (Date.now() - startedAt) / 1000,
        unexpected_console_errors: consoleErrors,
        ok: readOnlyOk,
      };
    }
    const pollingBudget = await probeTenMinutePolling(browser, server.origin, smokeCase.serverBasePath);
    const trialFeedback = await probeTrialFeedback(browser, server.origin, smokeCase.serverBasePath);
    const sessionIndexLease = await probeSessionIndexLease(
      browser,
      server.origin,
      smokeCase.serverBasePath,
    );

    const trialUrl = new URL(`${prefix}try/`, server.origin).href;
    const trialResponse = await page.goto(trialUrl, { waitUntil: "networkidle" });
    await page
      .locator("[data-testid='trial-profile'] option[value='python-cli']")
      .waitFor({ state: "attached" });
    const trialTitle = await page.title();
    const launchIdentityControls = page.locator(
      "[data-testid='trial-goal'], [data-testid='trial-token'], .trial-fields input, .trial-fields select",
    );
    const initialTrialFieldsEmpty =
      (await page.locator("[data-testid='trial-goal']").inputValue()) === "" &&
      (await page.locator("[data-testid='trial-executor-model']").inputValue()) === "" &&
      (await page.locator("[data-testid='trial-planner-model']").inputValue()) === "";
    await page.locator("[data-testid='check-contract']").click();
    const emptyGoalGuidance = await page.locator(".trial-error[role='alert']").innerText();
    await page.locator("[data-testid='trial-provider']").selectOption("lm-studio");
    const providerModelGuidance = await page
      .locator("[data-testid='trial-provider-model-hint']")
      .innerText();
    await page.locator("[data-testid='trial-provider']").selectOption("ollama");
    await page.locator("[data-testid='trial-goal']").fill("Create a CLI --pattern filter command");
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await page.locator("[data-testid='trial-executor-model']").fill(model);
    await page.locator("[data-testid='trial-planner-model']").fill(model);
    const desktopTrialAlignment = await trialControlAlignment(page);
    const requestLayout = await probeTrialLayout(
      page,
      "compose",
      "[data-testid='check-contract']",
      [],
    );
    await page.setViewportSize({ width: 390, height: 844 });
    const mobileTrialAlignment = await trialControlAlignment(page);
    await page.setViewportSize({ width: 1440, height: 1050 });
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    const gateOneLayout = await probeTrialLayout(
      page,
      "gate_1",
      "[data-testid='launch-session']",
      ["gate-one-card"],
    );
    const cardMarkdown = page.locator("[data-testid='gate-one-card-markdown']");
    await cardMarkdown.waitFor();
    const launch = page.locator("[data-testid='launch-session']");
    const launchDisabledBeforeConfirmation = await launch.isDisabled();
    const gateOneText = await page.locator("[data-testid='gate-one-card']").innerText();
    const cardMarkdownText = await cardMarkdown.innerText();
    const gateOneCopyIsPlain = [
      "Gate 1 — 実行前の確認",
      "必須チェック",
      "C1 — 実行動作",
      "C2 — ヘルプの正確さ",
      "C3 — 出力の正確さ",
      "C4 — 再現性",
      "全必須チェックに合格した実行: 3件中0件 (0%)",
    ].every((expected) => cardMarkdownText.includes(expected)) &&
      gateOneText.includes("時間と費用の目安") &&
      !gateOneText.includes("MEASURED PRICE TAG");
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
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-1-mobile.png`),
    });

    const injectedFailureMode = smokeCase.id === "proxy-commandagent" ? "access" : "network";
    await installPollFailure(page, injectedFailureMode);
    await page.locator("[data-testid='gate-one-confirm']").check();
    await launch.click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const gateTwoLayout = await probeTrialLayout(
      page,
      "gate_2",
      "[data-testid='session-progress'] .panel-heading",
      ["session-progress"],
    );
    const sessionId = await page.locator("[data-testid='session-progress'] h2").innerText();
    const gateTwoIdentityLocked = (await launchIdentityControls.count()) === 0;
    const tokenFocusAtGateTwo = {
      focused: false,
      stage: await page.locator("[data-testid='trial-stage-nav'] [aria-current='step'] strong").innerText(),
    };
    const degradedMonitor = page.locator("[data-testid='monitor-state'][data-monitor-status='degraded']");
    await degradedMonitor.waitFor();
    const degradedMonitorText = await degradedMonitor.innerText();
    const injectedFailureCount = await page.evaluate(
      () => window.__commandagentTrialPollInjection?.count ?? 0,
    );
    const connectedMonitor = page.locator(
      "[data-testid='monitor-state'][data-monitor-status='connected']",
    );
    await connectedMonitor.waitFor();
    const connectedMonitorText = await connectedMonitor.innerText();
    await page.locator("[data-testid='runtime-status'][data-session-state='running']").waitFor({ timeout: 10_000 });
    const runningRuntimeText = await page.locator("[data-testid='runtime-status']").innerText();
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
    const terminalLayout = await probeTrialLayout(
      page,
      "terminal",
      ".next-action-card .secondary-action",
      ["terminal-gate"],
    );
    const terminalIdentityLocked = (await launchIdentityControls.count()) === 0;
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
    const terminalHeading = await page
      .locator("[data-testid='terminal-result-heading']")
      .innerText();
    const expectedTerminalHeading = finalApi.body.gate === "gate_3"
      ? "すべての必須チェックに合格しました"
      : "すべての必須チェックには合格していません";
    const terminalHeadingIsPlain = terminalHeading === expectedTerminalHeading &&
      terminalHeading !== finalApi.body.assurance;
    const terminalVerdictSummary = await page
      .locator("[data-testid='terminal-verdict-summary']")
      .innerText();
    const terminalAssuranceSummary = await page
      .locator("[data-testid='terminal-assurance-summary']")
      .innerText();
    const terminalStatusSummary = await page
      .locator("[data-testid='terminal-status-summary']")
      .innerText();
    await page.locator("[data-testid='trial-session-files']").waitFor();
    await page.locator("[data-testid='trial-events-open']").click();
    await page.waitForFunction(
      () => document.querySelector("[data-testid='trial-file-viewer'] h2")?.textContent === "events.jsonl",
    );
    const eventsViewer = await page.locator("[data-testid='trial-file-viewer']").evaluate((viewer) => ({
      heading: viewer.querySelector("h2")?.textContent ?? "",
      path: viewer.querySelector("header code")?.textContent ?? "",
      content: viewer.querySelector("pre")?.textContent ?? "",
    }));
    await page.locator("[data-testid='trial-summary-open']").click();
    await page.waitForFunction(
      () => document.querySelector("[data-testid='trial-file-viewer'] h2")?.textContent === "summary.md",
    );
    const summaryViewer = await page.locator("[data-testid='trial-file-viewer']").evaluate((viewer) => ({
      heading: viewer.querySelector("h2")?.textContent ?? "",
      path: viewer.querySelector("header code")?.textContent ?? "",
      content: viewer.querySelector("pre")?.textContent ?? "",
    }));
    await page.locator("[data-testid='runtime-status'][data-session-state='idle']").waitFor({ timeout: 10_000 });
    const completedRuntimeText = await page.locator("[data-testid='runtime-status']").innerText();
    const sessionIndexCallStart = apiCalls.length;
    const sessionIndexResponse = page.waitForResponse((candidate) => {
      const url = new URL(candidate.url());
      return (
        candidate.request().method() === "GET" &&
        url.pathname.endsWith("/api/sessions") &&
        candidate.status() === 200
      );
    });
    await Promise.all([
      sessionIndexResponse,
      page.locator("[data-testid='refresh-trial-sessions']").click(),
    ]);
    const indexedSession = page.locator(".session-list li").filter({ hasText: sessionId }).first();
    await indexedSession.waitFor();
    await page.waitForFunction(
      ({ gate, id }) =>
        [...document.querySelectorAll(".session-list li")].some((row) => {
          const text = row.textContent ?? "";
          return text.includes(id) && text.includes(gate);
        }),
      { gate: finalApi.body.gate, id: sessionId },
    );
    const sessionIndexText = await indexedSession.innerText();
    const sessionIndexHref = await indexedSession
      .locator("[data-testid='session-reconnect-link']")
      .getAttribute("href");
    const sessionIndexCalls = apiCalls.slice(sessionIndexCallStart);
    const sessionIndexOnlyGets =
      sessionIndexCalls.length >= 1 &&
      sessionIndexCalls.every((call) => call.method === "GET");
    await page.setViewportSize({ width: 1440, height: 1050 });
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-terminal.png`),
    });

    const reconnectCallStart = apiCalls.length;
    await Promise.all([
      page.waitForNavigation({ waitUntil: "networkidle" }),
      indexedSession.locator("[data-testid='session-reconnect-link']").click(),
    ]);
    const sessionLinkCalls = apiCalls.slice(reconnectCallStart);
    const sessionLinkIssuedNoPost = sessionLinkCalls.every((call) => call.method !== "POST");
    const reloadSessionQuery = new URL(page.url()).searchParams.get("session");
    const reloadRestoredToken =
      (await page.locator("[data-testid='trial-token']").inputValue()) === trialCredential;
    await page.locator("[data-testid='trial-token']").fill(`${trialCredential}-wrong`);
    await page.locator("[data-testid='reconnect-session-button']").click();
    const authorizationGuidance = await page.locator(".trial-error[role='alert']").innerText();
    await page.waitForFunction(
      () => document.querySelector("[data-testid='trial-token']")?.value === "",
    );
    const rejectedTokenRemoved = await page.evaluate(
      () => !Object.values(sessionStorage).some((value) => value.includes("-wrong")),
    );
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await page.locator("[data-testid='reconnect-session-button']").click();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const reconnectCalls = apiCalls.slice(reconnectCallStart);
    const reconnectMethods = reconnectCalls.map((call) => call.method);
    const reconnectOnlyGets =
      reconnectMethods.length >= 2 && reconnectMethods.every((method) => method === "GET");
    const browserStorage = await page.evaluate(() => ({
      localStorageValues: Object.values(localStorage),
      sessionStorageEntries: Object.entries(sessionStorage),
      url: window.location.href,
    }));
    const expectedStorageKey = trialTokenStorageKey(smokeCase.serverBasePath);
    const tokenStayedTabScoped =
      !browserStorage.url.includes(trialCredential) &&
      browserStorage.localStorageValues.every((value) => !value.includes(trialCredential)) &&
      browserStorage.sessionStorageEntries.some(
        ([key, value]) => key === expectedStorageKey && value === trialCredential,
      );

    await page.goto(trialUrl, { waitUntil: "networkidle" });
    await installSessionConflict(page, sessionId);
    await page.locator("[data-testid='trial-goal']").fill("Create a CLI --pattern filter command");
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await page.locator("[data-testid='trial-executor-model']").fill(model);
    await page.locator("[data-testid='trial-planner-model']").fill(model);
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    const conflictGuidance = await page.locator(".trial-error[role='alert']").innerText();
    const conflictReconnectHref = await page
      .locator("[data-testid='reconnect-session-link']")
      .getAttribute("href");
    const conflictReconnectId = conflictReconnectHref === null
      ? null
      : new URL(conflictReconnectHref, page.url()).searchParams.get("session");
    const conflictSessionQuery = new URL(page.url()).searchParams.get("session");
    const conflictDispatchCount = await page.evaluate(
      () => window.__commandagentTrialConflictInjection?.count ?? 0,
    );

    const mobile = await probeMobile(browser, server.origin, smokeCase.serverBasePath);

    const eventsPath = join(executionRoot, ".anvil", "runs", sessionId, "events.jsonl");
    const eventBytes = await readFile(eventsPath);
    await writeFile(join(outputDirectory, `${smokeCase.id}-events.jsonl`), eventBytes);
    const lifecycleUrl = new URL(trialUrl);
    lifecycleUrl.searchParams.set("session", sessionId);
    await page.goto(lifecycleUrl.href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='reconnect-session']").fill(sessionId);
    await page.locator("[data-testid='trial-token']").fill(trialCredential);
    await page.locator("[data-testid='reconnect-session-button']").click();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await page.waitForTimeout(1_000);
    await page.locator("[data-testid='close-session']").click();
    await page.locator("[data-testid='closed-session']").waitFor();
    const closedIdentityLocked = (await launchIdentityControls.count()) === 0;
    await page.locator("[data-testid='start-new-run']").click();
    const newRunStage = await page.locator(".gate-chip").innerText();
    const newRunIdentityEditable = await allEnabled(launchIdentityControls, 6);
    const previousRunCleared =
      (await page.locator("[data-testid='session-progress']").count()) === 0 &&
      (await page.locator("[data-testid='terminal-gate']").count()) === 0 &&
      (await page.locator("[data-testid='gate-one-card']").count()) === 0;
    await page.locator("[data-testid='trial-goal']").fill("Create a CLI --pattern filter command");
    await page.locator("[data-testid='trial-executor-model']").fill(model);
    await page.locator("[data-testid='trial-planner-model']").fill(model);
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const nextSessionId = await page.locator("[data-testid='session-progress'] h2").innerText();
    await page.locator("[data-testid='terminal-gate']").waitFor({ timeout: trialTimeoutMs });
    const nextSessionTerminalLabel = await page
      .locator("[data-testid='terminal-gate'] .verdict-card .panel-index")
      .innerText();
    const nextSessionReachedTerminal = ["GATE 3", "GATE 4"].some(
      (gate) => nextSessionTerminalLabel.toUpperCase().includes(gate),
    );
    await page.waitForTimeout(1_000);
    const apiLog = {
      schema_version: "commandagent.gui-api-smoke/v1",
      base_path: smokeCase.buildBasePath,
      denied_without_confirmation: deniedWithoutConfirmation,
      observed_calls: apiCalls,
      ten_minute_polling: pollingBudget,
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
      session_index: {
        calls: sessionIndexCalls,
        href: sessionIndexHref,
        link_calls: sessionLinkCalls,
        link_issued_no_post: sessionLinkIssuedNoPost,
        only_gets: sessionIndexOnlyGets,
        visible_text: sessionIndexText,
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
    const layoutChecks = [requestLayout, gateOneLayout, gateTwoLayout, terminalLayout];

    const ok =
      dashboardOk &&
      dashboardAccessible &&
      assets.status === 200 &&
      assets.headingMatches &&
      assets.titleMatches &&
      measurements.status === 200 &&
      measurements.headingMatches &&
      measurements.titleMatches &&
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      readOnlyUi.ok &&
      runDetail.titleMatches &&
      trialResponse?.status() === 200 &&
      trialTitle === "トライアル | CommandAgent" &&
      initialTrialFieldsEmpty &&
      emptyGoalGuidance.includes("目標を入力してください") &&
      providerModelGuidance.includes("実行モデルは自動更新されません") &&
      providerModelGuidance.includes("LM Studio") &&
      desktopTrialAlignment.aligned &&
      mobileTrialAlignment.aligned &&
      launchDisabledBeforeConfirmation &&
      gateOneCopyIsPlain &&
      gateTwoIdentityLocked &&
      !tokenFocusAtGateTwo.focused &&
      tokenFocusAtGateTwo.stage === "実行" &&
      terminalIdentityLocked &&
      closedIdentityLocked &&
      newRunStage === "下書き" &&
      newRunIdentityEditable &&
      previousRunCleared &&
      nextSessionId !== sessionId &&
      nextSessionReachedTerminal &&
      deniedWithoutConfirmation.status === 428 &&
      pollingBudget.ok &&
      trialFeedback.ok &&
      sessionIndexLease.ok &&
      executionScroll.clearsStickyHeader &&
      terminalScroll.clearsStickyHeader &&
      finalApi.status === 200 &&
      ["gate_3", "gate_4"].includes(finalApi.body.gate) &&
      terminalHeadingIsPlain &&
      terminalText.includes("結果") &&
      terminalText.includes("保証水準") &&
      terminalText.includes("状態") &&
      terminalVerdictSummary.includes("最終受け入れ") &&
      terminalAssuranceSummary.length > 0 &&
      terminalStatusSummary.length > 0 &&
      eventsViewer.heading === "events.jsonl" &&
      eventsViewer.path === "events.jsonl" &&
      eventsViewer.content.includes('"event"') &&
      summaryViewer.heading === "summary.md" &&
      summaryViewer.path === "summary.md" &&
      summaryViewer.content.trim() !== "" &&
      injectedFailureCount === 1 &&
      degradedMonitorText.includes(
        injectedFailureMode === "access" ? "上流アクセス" : "プロキシまたはネットワーク",
      ) &&
      connectedMonitorText.includes("監視: 接続中") &&
      connectedMonitorText.includes("最終更新成功:") &&
      idleRuntimeText.includes("Trial 利用可") &&
      idleRuntimeText.includes("実行中なし") &&
      runningRuntimeText.includes(`実行中 ${sessionId.slice(0, 8)}`) &&
      completedRuntimeText.includes("実行中なし") &&
      sessionIndexText.includes(sessionId) &&
      sessionIndexText.includes("開始:") &&
      sessionIndexText.includes("最終更新:") &&
      sessionIndexText.includes(finalApi.body.gate.toUpperCase()) &&
      new URL(sessionIndexHref, trialUrl).searchParams.get("session") === sessionId &&
      sessionIndexOnlyGets &&
      sessionLinkIssuedNoPost &&
      reloadSessionQuery === sessionId &&
      reloadRestoredToken &&
      rejectedTokenRemoved &&
      authorizationGuidance.includes("Trial アクセストークン") &&
      reconnectOnlyGets &&
      tokenStayedTabScoped &&
      conflictGuidance.includes(`セッション ${sessionId} に再接続`) &&
      conflictReconnectId === sessionId &&
      conflictSessionQuery === sessionId &&
      conflictDispatchCount === 1 &&
      mobile.ok &&
      layoutChecks.every((check) => check.ok) &&
      expectedNegativeConsoleErrors.some((entry) => entry.includes("status of 428")) &&
      expectedNegativeConsoleErrors.some((entry) => entry.includes("status of 401")) &&
      unexpectedConsoleErrors.length === 0;
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      dashboard,
      api_checks: apiChecks,
      svg: map,
      links_use_base_path: linksUseBasePath,
      run_ledger_accessibility: runLedgerAccessibility,
      pages: { assets, measurements, run_detail: runDetail, trial: { status: trialResponse?.status() ?? 0, title: trialTitle } },
      issue_75: readOnlyUi,
      mobile,
      ten_minute_polling: pollingBudget,
      trial_feedback: trialFeedback,
      session_index_lease: sessionIndexLease,
      layout: {
        viewport: { width: 390, height: 844 },
        states: layoutChecks,
      },
      gate_1: {
        card_markdown_visible_text: cardMarkdownText,
        copy_is_plain_japanese: gateOneCopyIsPlain,
        empty_goal_guidance: emptyGoalGuidance,
        initial_fields_empty: initialTrialFieldsEmpty,
        launch_disabled_before_confirmation: launchDisabledBeforeConfirmation,
        provider_model_guidance: providerModelGuidance,
        api_without_confirmation_status: deniedWithoutConfirmation.status,
        control_alignment: {
          desktop_1440: desktopTrialAlignment,
          mobile_390: mobileTrialAlignment,
        },
        visible_text: gateOneText,
      },
      lifecycle: {
        gate_2_identity_locked: gateTwoIdentityLocked,
        gate_2_token_focus: tokenFocusAtGateTwo,
        terminal_identity_locked: terminalIdentityLocked,
        closed_identity_locked: closedIdentityLocked,
        new_run_stage: newRunStage,
        new_run_identity_editable: newRunIdentityEditable,
        previous_run_cleared: previousRunCleared,
        next_session_id: nextSessionId,
        next_session_reached_terminal: nextSessionReachedTerminal,
      },
      session: {
        id: sessionId,
        gate: finalApi.body.gate,
        status: finalApi.body.status,
        verdict: finalApi.body.verdict,
        assurance: finalApi.body.assurance,
        event_count: finalApi.body.event_count,
        events_sha256: `sha256:${createHash("sha256").update(eventBytes).digest("hex")}`,
        terminal_heading: terminalHeading,
        terminal_heading_is_plain_japanese: terminalHeadingIsPlain,
        terminal_verdict_summary: terminalVerdictSummary,
        terminal_assurance_summary: terminalAssuranceSummary,
        terminal_status_summary: terminalStatusSummary,
        mobile_stage_scroll: {
          execution: executionScroll,
          terminal: terminalScroll,
        },
        events_viewer: {
          heading: eventsViewer.heading,
          path: eventsViewer.path,
          content_bytes: Buffer.byteLength(eventsViewer.content),
          contains_event: eventsViewer.content.includes('"event"'),
        },
        summary_viewer: {
          heading: summaryViewer.heading,
          path: summaryViewer.path,
          content_bytes: Buffer.byteLength(summaryViewer.content),
        },
        terminal_visible_text: terminalText,
      },
      monitoring: {
        failure_mode: injectedFailureMode,
        injected_failure_count: injectedFailureCount,
        degraded_visible_text: degradedMonitorText,
        connected_visible_text: connectedMonitorText,
      },
      runtime_status: {
        completed_visible_text: completedRuntimeText,
        idle_visible_text: idleRuntimeText,
        running_visible_text: runningRuntimeText,
      },
      session_index: {
        calls: sessionIndexCalls,
        href: sessionIndexHref,
        link_calls: sessionLinkCalls,
        link_issued_no_post: sessionLinkIssuedNoPost,
        only_gets: sessionIndexOnlyGets,
        visible_text: sessionIndexText,
      },
      reconnect: {
        reload_session_query: reloadSessionQuery,
        token_restored_after_reload: reloadRestoredToken,
        rejected_token_removed: rejectedTokenRemoved,
        authorization_guidance: authorizationGuidance,
        calls: reconnectCalls,
        only_gets: reconnectOnlyGets,
        storage_key: expectedStorageKey,
        token_stayed_tab_scoped: tokenStayedTabScoped,
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
  } catch (reason) {
    const pageError = await page.locator(".trial-error[role='alert']").textContent().catch(() => null);
    const stage = await page.locator(".gate-chip").textContent().catch(() => null);
    await page
      .screenshot({
        fullPage: true,
        path: join(outputDirectory, `${smokeCase.id}-failure.png`),
      })
      .catch(() => undefined);
    throw new Error(
      `${message(reason)}\nPage diagnostics: ${JSON.stringify({ page_error: pageError, stage, url: page.url() })}`,
    );
  } finally {
    await browser.close();
    server.stop();
  }
}

async function probeReadOnlyUi(page, origin, basePath, runSummaries, caseId) {
  const measurements = await probePage(
    page,
    origin,
    basePath,
    "measurements/",
    "計測",
    "計測 | CommandAgent",
  );
  await page.setViewportSize({ width: 390, height: 844 });
  const mapFrame = page.locator("[data-testid='measurement-map-frame']");
  await mapFrame.waitFor();
  const mobileMap = await mapFrame.evaluate((frame) => ({
    client_width: frame.clientWidth,
    horizontally_scrollable: frame.scrollWidth > frame.clientWidth,
    scroll_width: frame.scrollWidth,
  }));
  const mobilePageFits = await page.evaluate(
    () => document.documentElement.scrollWidth <= window.innerWidth,
  );
  const fullSizeHref = await page.locator(".map-source-link").getAttribute("href");
  await page.screenshot({
    fullPage: true,
    path: join(outputDirectory, `${caseId}-measurements-mobile.png`),
  });

  await page.setViewportSize({ width: 1440, height: 1050 });
  const runDetail = await probePage(
    page,
    origin,
    basePath,
    "runs/",
    "検証・運用レポート",
    "検証・運用レポート | CommandAgent",
  );
  await page.waitForFunction(
    () => document.querySelectorAll("#run-select option:not([value=''])").length > 0,
  );
  const unselectedText = await page.locator(".run-document").innerText();
  const displayedOptions = await page.locator("#run-select option:not([value=''])").evaluateAll(
    (options) => options.map((option) => ({ text: option.textContent ?? "", value: option.value })),
  );
  const expectedOptions = await page.evaluate((runs) => {
    const formatter = new Intl.DateTimeFormat("ja-JP", {
      dateStyle: "medium",
      timeStyle: "short",
    });
    return runs.map((run) => {
      const date = run.modified_epoch_seconds === 0
        ? "時刻不明"
        : formatter.format(new Date(run.modified_epoch_seconds * 1000));
      return { text: `${date} — ${run.status_text} — ${run.id}`, value: run.id };
    });
  }, runSummaries);
  const optionsIncludeDatesAndStatus =
    JSON.stringify(displayedOptions) === JSON.stringify(expectedOptions);
  const firstRunId = runSummaries[0]?.id ?? "";
  if (firstRunId === "") throw new Error("Run detail probe requires at least one run");
  const filterInput = page.locator("#run-filter");
  await filterInput.fill("__issue_75_no_match__");
  const noMatchLabelVisible = await page
    .locator(".run-picker .state-code", { hasText: "該当なし" })
    .isVisible();
  await filterInput.fill(firstRunId);
  const filteredOptions = await page.locator("#run-select option:not([value=''])").evaluateAll(
    (options) => options.map((option) => ({ text: option.textContent ?? "", value: option.value })),
  );
  const filterMatchesId =
    filteredOptions.length === 1 && filteredOptions[0]?.value === firstRunId;
  await page.locator("#run-select").selectOption(firstRunId);
  await page.locator(".document-viewer").waitFor();

  const content = page.locator("[data-testid='document-content']");
  const toggle = page.locator("[data-testid='document-wrap-toggle']");
  const sourceLink = page.locator("[data-testid='document-source-link']");
  const sourceHref = await sourceLink.getAttribute("href");
  const sourceTarget = await sourceLink.getAttribute("target");
  const sourceLinkPresent =
    sourceHref?.endsWith(`/api/runs/${encodeURIComponent(firstRunId)}`) === true &&
    sourceTarget === "_blank";
  const initialClass = await content.getAttribute("class");
  const initialPressed = await toggle.getAttribute("aria-pressed");
  await toggle.click();
  await page.waitForFunction(
    () => document.querySelector("[data-testid='document-content']")
      ?.classList.contains("document-content--unwrapped"),
  );
  const toggledClass = await content.getAttribute("class");
  const toggledPressed = await toggle.getAttribute("aria-pressed");
  await toggle.click();
  await page.waitForFunction(
    () => document.querySelector("[data-testid='document-content']")
      ?.classList.contains("document-content--wrapped"),
  );
  const restoredClass = await content.getAttribute("class");
  const restoredPressed = await toggle.getAttribute("aria-pressed");
  const wrapToggle = {
    classes_switch:
      initialClass === "document-content--wrapped" &&
      toggledClass === "document-content--unwrapped" &&
      restoredClass === "document-content--wrapped",
    initial_class: initialClass,
    initial_pressed: initialPressed,
    restored_class: restoredClass,
    restored_pressed: restoredPressed,
    toggled_class: toggledClass,
    toggled_pressed: toggledPressed,
  };
  await page.screenshot({
    fullPage: true,
    path: join(outputDirectory, `${caseId}-run-detail.png`),
  });

  const fullSizeLinkPresent = fullSizeHref?.endsWith("/api/maps/score-time.svg") ?? false;
  const unselectedHasNoRecords = !unselectedText.includes("NO RECORDS");
  return {
    pages: { measurements, run_detail: runDetail },
    run_selection: {
      displayed_options: displayedOptions.length,
      expected_options: expectedOptions.length,
      filter_matches_id: filterMatchesId,
      no_records_label_absent: unselectedHasNoRecords,
      no_match_label_visible: noMatchLabelVisible,
      options_include_dates_and_status: optionsIncludeDatesAndStatus,
    },
    source_link: {
      href: sourceHref,
      opens_new_tab: sourceTarget === "_blank",
      present: sourceLinkPresent,
    },
    wrap_toggle: wrapToggle,
    mobile_map: {
      ...mobileMap,
      full_size_href: fullSizeHref,
      full_size_link_present: fullSizeLinkPresent,
      page_fits_viewport: mobilePageFits,
      viewport_width: 390,
    },
    ok:
      measurements.status === 200 &&
      measurements.headingMatches &&
      measurements.titleMatches &&
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      runDetail.titleMatches &&
      unselectedHasNoRecords &&
      noMatchLabelVisible &&
      filterMatchesId &&
      optionsIncludeDatesAndStatus &&
      sourceLinkPresent &&
      wrapToggle.classes_switch &&
      initialPressed === "true" &&
      toggledPressed === "false" &&
      restoredPressed === "true" &&
      mobileMap.horizontally_scrollable &&
      mobilePageFits &&
      fullSizeLinkPresent,
  };
}

async function probeTrialFeedback(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const sessionId = "0198b9c8-fab8-7000-8000-000000000069";
  let phaseTotal = 0;
  let terminal = false;
  try {
    await page.clock.install({ time: new Date("2026-08-16T00:00:00Z") });
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      if (pathname.endsWith("/api/trial-workspace")) {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify({ status: "idle" }),
        });
        return;
      }
      if (pathname.endsWith("/api/session-proposals") && request.method() === "POST") {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify(syntheticFeedbackProposal()),
        });
        return;
      }
      if (pathname.endsWith("/api/sessions") && request.method() === "POST") {
        await route.fulfill({
          contentType: "application/json",
          status: 202,
          body: JSON.stringify({
            id: sessionId,
            gate: "gate_2",
            status: "starting",
            events_path: `.anvil/runs/${sessionId}/events.jsonl`,
          }),
        });
        return;
      }
      if (pathname.endsWith(`/api/sessions/${sessionId}`) && request.method() === "GET") {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify(syntheticFeedbackSession(sessionId, phaseTotal, terminal)),
        });
        return;
      }
      await route.continue();
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-goal']").fill("Synthetic Gate 2 feedback probe");
    await page.locator("[data-testid='trial-token']").fill("synthetic-feedback-token");
    await page.locator("[data-testid='trial-executor-model']").fill("synthetic-model");
    await page.locator("[data-testid='trial-planner-model']").fill("synthetic-model");
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const zeroTotalHidden =
      (await page.locator("[data-testid='phase-progress']").count()) === 0;

    phaseTotal = 5;
    await page.clock.runFor(1_100);
    await page.locator("[data-testid='phase-progress']").getByText("フェーズ 2 / 5").waitFor();
    const elapsed = page.locator("[data-testid='elapsed-time']");
    const elapsedBefore = Number(await elapsed.getAttribute("data-elapsed-seconds"));
    const elapsedTextBefore = await elapsed.locator("strong").innerText();
    const runningTitle = await page.title();
    await page.clock.runFor(2_200);
    const elapsedAfter = Number(await elapsed.getAttribute("data-elapsed-seconds"));
    const elapsedTextAfter = await elapsed.locator("strong").innerText();
    const phaseText = await page.locator("[data-testid='phase-progress'] strong").innerText();
    const meanText = await page
      .locator("[data-testid='mean-duration-comparison'] strong")
      .innerText();
    const meanLabel = await page
      .locator("[data-testid='mean-duration-comparison'] span")
      .innerText();
    const feedbackAfterMonitor =
      await page.locator("[data-testid='monitor-state'] + [data-testid='execution-feedback']").count();

    terminal = true;
    await page.clock.runFor(1_100);
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await page.waitForFunction((title) => document.title !== title, runningTitle);
    const terminalTitle = await page.title();
    const elapsedChanged =
      elapsedAfter >= elapsedBefore + 2 && elapsedTextAfter !== elapsedTextBefore;
    const titleChanged =
      terminalTitle !== runningTitle &&
      terminalTitle === "✔ すべての必須チェックに合格しました — CommandAgent";
    return {
      elapsed_before_seconds: elapsedBefore,
      elapsed_after_seconds: elapsedAfter,
      elapsed_before_text: elapsedTextBefore,
      elapsed_after_text: elapsedTextAfter,
      elapsed_changed: elapsedChanged,
      zero_total_hidden: zeroTotalHidden,
      phase_text: phaseText,
      phase_uses_total: phaseText === "フェーズ 2 / 5",
      measured_mean_text: meanText,
      measured_mean_visible: meanText === "平均 10.2 分",
      mean_is_not_eta: meanLabel.includes("予測ではありません"),
      monitor_and_progress_separate: feedbackAfterMonitor === 1,
      running_title: runningTitle,
      terminal_title: terminalTitle,
      title_changed: titleChanged,
      ok:
        elapsedChanged &&
        zeroTotalHidden &&
        phaseText === "フェーズ 2 / 5" &&
        meanText === "平均 10.2 分" &&
        meanLabel.includes("予測ではありません") &&
        feedbackAfterMonitor === 1 &&
        titleChanged,
    };
  } finally {
    await page.close();
  }
}

async function probeSessionIndexLease(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const sessionId = "0198b9c8-fab8-7000-8000-000000000071";
  let dispatchCount = 0;
  try {
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      if (pathname.endsWith("/api/sessions") && request.method() === "GET") {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify({
            sessions: [
              {
                id: sessionId,
                started_epoch_seconds: 1_723_769_600,
                modified_epoch_seconds: 1_723_769_660,
                gate: "gate_2",
                status: "running",
              },
            ],
            lease: { status: "running", session_id: sessionId },
          }),
        });
        return;
      }
      if (pathname.endsWith("/api/trial-workspace") && request.method() === "GET") {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify({ status: "running", session_id: sessionId }),
        });
        return;
      }
      if (pathname.endsWith("/api/session-proposals") && request.method() === "POST") {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify(syntheticProposal()),
        });
        return;
      }
      if (pathname.endsWith("/api/sessions") && request.method() === "POST") {
        dispatchCount += 1;
        await route.fulfill({ status: 500, body: "unexpected dispatch" });
        return;
      }
      await route.continue();
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-goal']").fill("Synthetic running lease probe");
    await page
      .locator("[data-testid='trial-token']")
      .fill("synthetic-session-index-lease-token-000000000071");
    await page.locator("[data-testid='trial-executor-model']").fill("synthetic-model");
    await page.locator("[data-testid='trial-planner-model']").fill("synthetic-model");
    await page.locator("[data-testid='session-reconnect-link']").waitFor();
    const leaseText = await page.locator("[data-testid='workspace-lease-status']").innerText();
    const sessionText = await page.locator("[data-testid='trial-session-index']").innerText();
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    const launch = page.locator("[data-testid='launch-session']");
    const launchDisabled = await launch.isDisabled();
    const reason = await page.locator("[data-testid='launch-block-reason']").innerText();
    return {
      dispatch_count: dispatchCount,
      launch_disabled: launchDisabled,
      lease_text: leaseText,
      reason,
      session_text: sessionText,
      ok:
        leaseText.includes("実行中") &&
        leaseText.includes(sessionId) &&
        sessionText.includes(sessionId) &&
        sessionText.includes("GATE_2 / RUNNING") &&
        launchDisabled &&
        reason.includes(sessionId) &&
        reason.includes("新しい起動はできません") &&
        dispatchCount === 0,
    };
  } finally {
    await page.close();
  }
}

async function probeTenMinutePolling(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const durationMs = 600_000;
  const fixedIntervalMs = 750;
  const fixedIntervalCalls = 1 + Math.floor(durationMs / fixedIntervalMs);
  const sessionId = "0198b9c8-fab8-7000-8000-000000000080";
  const etag = 'W/"synthetic-unchanged"';
  const observedCalls = [];
  try {
    await page.clock.install({ time: new Date("2026-08-16T00:00:00Z") });
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      if (pathname.endsWith("/api/trial-workspace")) {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify({ status: "idle" }),
        });
        return;
      }
      if (pathname.endsWith("/api/session-proposals")) {
        await route.fulfill({
          contentType: "application/json",
          status: 200,
          body: JSON.stringify(syntheticProposal()),
        });
        return;
      }
      if (pathname.endsWith("/api/sessions") && request.method() === "POST") {
        await route.fulfill({
          contentType: "application/json",
          status: 202,
          body: JSON.stringify({
            id: sessionId,
            gate: "gate_2",
            status: "starting",
            events_path: `.anvil/runs/${sessionId}/events.jsonl`,
          }),
        });
        return;
      }
      if (pathname.endsWith(`/api/sessions/${sessionId}`) && request.method() === "GET") {
        observedCalls.push({
          sequence: observedCalls.length + 1,
          if_none_match: request.headers()["if-none-match"] ?? null,
        });
        if (observedCalls.length === 1) {
          await route.fulfill({
            contentType: "application/json",
            headers: { etag, "cache-control": "no-cache" },
            status: 200,
            body: JSON.stringify({
              id: sessionId,
              gate: "gate_2",
              status: "running",
              verdict: null,
              assurance: null,
              phases: [],
              event_count: 0,
              acceptance_sheet: null,
              section5: null,
              events_path: `.anvil/runs/${sessionId}/events.jsonl`,
            }),
          });
        } else {
          await route.fulfill({ status: 304, headers: { etag, "cache-control": "no-cache" } });
        }
        return;
      }
      await route.continue();
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-goal']").fill("Synthetic ten-minute polling probe");
    await page.locator("[data-testid='trial-token']").fill("synthetic-poll-token");
    await page.locator("[data-testid='trial-executor-model']").fill("synthetic-model");
    await page.locator("[data-testid='trial-planner-model']").fill("synthetic-model");
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    await page.locator("[data-testid='session-progress']").waitFor();
    await page.waitForFunction(() => document.body.textContent?.includes("running"));
    for (let elapsed = 0; elapsed < durationMs; elapsed += 1_000) {
      await page.clock.runFor(1_000);
      await new Promise((resolveTurn) => setImmediate(resolveTurn));
    }

    const conditionalRequests = observedCalls
      .slice(1)
      .every((call) => call.if_none_match === etag);
    const reductionPercent = 100 * (1 - observedCalls.length / fixedIntervalCalls);
    return {
      duration_ms: durationMs,
      fixed_750ms_calls: fixedIntervalCalls,
      observed_call_count: observedCalls.length,
      observed_calls: observedCalls,
      conditional_requests: conditionalRequests,
      reduction_percent: reductionPercent,
      ok:
        observedCalls.length >= 50 &&
        observedCalls.length <= 65 &&
        conditionalRequests &&
        reductionPercent >= 90,
    };
  } finally {
    await page.close();
  }
}

function syntheticFeedbackProposal() {
  const proposal = syntheticProposal();
  return {
    ...proposal,
    card_hash: `sha256:${"6".repeat(64)}`,
    identity: {
      ...proposal.identity,
      request: "Synthetic Gate 2 feedback probe",
      workspace: "/synthetic/feedback-probe",
      contract_ref: "synthetic/feedback",
      contract_checks: ["elapsed and phase feedback"],
      band_measurement: "mocked PolledSession",
      full_meaning: "The feedback probe does not delegate a CLI process.",
    },
    price: {
      ...proposal.price,
      duration_n: 5,
      average_duration_seconds: 612,
    },
  };
}

function syntheticFeedbackSession(sessionId, phaseTotal, terminal) {
  return {
    id: sessionId,
    gate: terminal ? "gate_3" : "gate_2",
    status: terminal ? "completed" : "running",
    verdict: terminal ? "pass" : null,
    assurance: terminal ? "full" : null,
    phases: [
      { id: "prepare", index: 1, total: phaseTotal, stage: "complete", status: "completed" },
      {
        id: "implement",
        index: 2,
        total: phaseTotal,
        stage: terminal ? "complete" : "execute",
        status: terminal ? "completed" : "running",
      },
    ],
    event_count: terminal ? 8 : 3,
    acceptance_sheet: terminal ? "# Synthetic acceptance\\n\\nPASS" : null,
    section5: terminal ? "PASS" : null,
    events_path: `.anvil/runs/${sessionId}/events.jsonl`,
  };
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

async function allEnabled(locator, expectedCount) {
  return locator.evaluateAll(
    (controls, count) =>
      controls.length === count && controls.every((control) => control.disabled === false),
    expectedCount,
  );
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
    const dashboardIntroOneLine = await page.locator(".page-intro > p").isHidden();
    const dashboardFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    const trial = await page.goto(new URL(`${prefix}try/`, origin).href, {
      waitUntil: "networkidle",
    });
    const trialHeading = await page.locator("h1").innerText();
    const trialIntroOneLine = await page.locator(".page-intro > p").isHidden();
    await page.locator("[data-testid='reconnect-card']").waitFor();
    const trialFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    return {
      dashboard: { fits_viewport: dashboardFits, heading: dashboardHeading, intro_one_line: dashboardIntroOneLine, status: dashboard?.status() ?? 0 },
      trial: { fits_viewport: trialFits, heading: trialHeading, intro_one_line: trialIntroOneLine, status: trial?.status() ?? 0 },
      ok:
        dashboard?.status() === 200 &&
        dashboardHeading === "概要" &&
        dashboardIntroOneLine &&
        dashboardFits &&
        trial?.status() === 200 &&
        trialHeading === "トライアル" &&
        trialIntroOneLine &&
        trialFits,
    };
  } finally {
    await page.close();
  }
}

function syntheticProposal() {
  return {
    confirmation_required: true,
    card_hash: `sha256:${"8".repeat(64)}`,
    card_markdown: "# Synthetic Gate 1 polling probe",
    identity: {
      request: "Synthetic ten-minute polling probe",
      workspace: "/synthetic/polling-probe",
      profile: "python-cli",
      intent: "create",
      task_family: "cli",
      route_bases: ["smoke=synthetic"],
      contract_ref: "synthetic/polling",
      contract_checks: ["conditional status polling"],
      band_full: 1,
      band_denominator: 1,
      band_rate: "1/1",
      band_arm: "smoke",
      band_measurement: "virtual clock",
      band_source: "gui/scripts/smoke.mjs",
      full_meaning: "The exported page keeps one representation and revalidates it.",
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
          JSON.stringify({
            code: "trial_workspace_running",
            error: `trial workspace is already running session ${activeSessionId}`,
          }),
          { headers: { "content-type": "application/json" }, status: 409 },
        );
      }
      return nativeFetch(input, init);
    };
  }, sessionId);
}

async function probeTrialLayout(page, expectedStage, primarySelector, expectedVisibleStateIds) {
  const previousViewport = page.viewportSize();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator("[data-testid='trial-active-stage']").waitFor();
  await page.waitForTimeout(450);

  const stage = await page.locator("[data-testid='trial-active-stage']").getAttribute("data-stage");
  const stepLabels = await page.locator("[data-testid='trial-stage-nav'] strong").allInnerTexts();
  const primary = page.locator(primarySelector).first();
  await primary.waitFor();
  const primaryBox = await primary.boundingBox();
  const bottomNavigationBox = await page.locator(".sidebar").boundingBox();
  const primaryInInitialViewport =
    primaryBox !== null &&
    primaryBox.y >= 0 &&
    primaryBox.y + primaryBox.height <= 844 &&
    (bottomNavigationBox === null || primaryBox.y + primaryBox.height <= bottomNavigationBox.y);
  const visibleStateIds = await page
    .locator(
      "[data-testid='gate-one-card'], [data-testid='session-progress'], [data-testid='terminal-gate']",
    )
    .evaluateAll((elements) =>
      elements
        .filter((element) => {
          const style = window.getComputedStyle(element);
          return style.display !== "none" && style.visibility !== "hidden" && element.getClientRects().length > 0;
        })
        .map((element) => element.getAttribute("data-testid")),
    );

  if (previousViewport !== null) await page.setViewportSize(previousViewport);
  const expectedLabels = ["依頼", "確認", "実行", "結果"];
  const oneStateVisible =
    visibleStateIds.length === expectedVisibleStateIds.length &&
    expectedVisibleStateIds.every((id) => visibleStateIds.includes(id));
  return {
    stage,
    step_labels: stepLabels,
    visible_state_ids: visibleStateIds,
    primary_in_initial_viewport: primaryInInitialViewport,
    one_state_visible: oneStateVisible,
    ok:
      stage === expectedStage &&
      stepLabels.join("\u0000") === expectedLabels.join("\u0000") &&
      primaryInInitialViewport &&
      oneStateVisible,
  };
}

async function probePage(page, origin, basePath, relativePath, expectedHeading, expectedTitle) {
  const prefix = displayBasePath(basePath);
  const url = new URL(`${prefix}${relativePath}`, origin).href;
  const response = await page.goto(url, { waitUntil: "networkidle" });
  const heading = await page.locator("h1").innerText();
  const title = await page.title();
  return {
    status: response?.status() ?? 0,
    heading,
    headingMatches: heading === expectedHeading,
    title,
    titleMatches: title === expectedTitle,
  };
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

function trialTokenStorageKey(basePath) {
  return `commandagent.gui.trial-token:${basePath}`;
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
