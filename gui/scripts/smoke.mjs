import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { chmod, cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
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
const wizardOnly = arguments_.includes("--wizard-only");
const gateOneOnly = arguments_.includes("--gate-one-only");
const outputDirectory = valueArgument(arguments_, "--output");
const feedbackOnly = arguments_.includes("--feedback-only");
const pollingOnly = arguments_.includes("--polling-only");
const providerOnly = arguments_.includes("--provider-only");
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
const expectedNextjsAcmeHash = "sha256:6dab3671f1750a85830185486cf94f199b227cd4f3d4eccfe03a30742cee7ac0";
const managedPlaywrightPath =
  process.env.COMMANDAGENT_PLAYWRIGHT_PATH ??
  join(homedir(), ".anvil", "tools", "interaction-probe", "node_modules", "playwright");
const helpMapEntries = [
  {
    copy: "前提を確認し、サンプル目標から Gate 1 の実行前確認を試せます。",
    owner: "getting-started-gui.md#はじめに",
    source: "gui/components/getting-started.tsx",
  },
  {
    copy: "CLI を動かす前に、目標・変更範囲・検証条件を確認する段階です。",
    owner: "getting-started-gui.md#terms-shown-in-the-app",
    source: "gui/components/getting-started.tsx",
  },
  {
    copy: "Trial がファイルを変更できる、専用の作業ディレクトリです。",
    owner: "getting-started-gui.md#terms-shown-in-the-app",
    source: "gui/components/getting-started.tsx",
  },
  {
    copy: "目標に追加する検証知識。選択した版とハッシュが確認内容に固定されます。",
    owner: "gui-trial.md#pack-selection-and-frozen-identity",
    source: "gui/components/getting-started.tsx",
  },
  {
    copy: "Gate 1 は CLI 実行前の確認です",
    owner: "gui-trial.md#gate-1-confirm-before-execution",
    source: "gui/components/trial-run.tsx",
  },
  {
    copy: "固定済みパックが見つかりません。",
    owner: "gui-extensions.md#extensions-catalog",
    source: "gui/app/assets/page.tsx",
  },
  {
    copy: "Trial で使う",
    owner: "gui-extensions.md#extensions-catalog",
    source: "gui/app/assets/page.tsx",
  },
  {
    copy: "pack 作成ウィザードを開く",
    owner: "gui-extensions.md#pack-creation-wizard",
    source: "gui/components/pack-wizard.tsx",
  },
  {
    copy: "確認済み GUI Trial セッションはありません。",
    owner: "gui-history.md#session-rows-and-refresh",
    source: "gui/components/trial-session-index.tsx",
  },
];

if (outputDirectory === null) {
  console.error(
    "usage: npm run smoke -- --output <evidence-directory> [--read-only | --overview-only | --wizard-only | --gate-one-only | --feedback-only | --polling-only | --provider-only] [--commandagent-bin <path>] [--model <name>]",
  );
  process.exit(2);
}
if (!Number.isFinite(trialTimeoutMs) || trialTimeoutMs <= 0) {
  console.error("--trial-timeout-ms must be a positive number");
  process.exit(2);
}

const helpMapMarkdown = await readFile(join(repositoryRoot, "docs/user/gui-help-map.md"), "utf8");
const helpMapChecks = await Promise.all(
  helpMapEntries.map(async (entry) => {
    const source = await readFile(join(repositoryRoot, entry.source), "utf8");
    return {
      ...entry,
      app_source_count: countOccurrences(source, entry.copy),
      map_count: countOccurrences(helpMapMarkdown, entry.copy),
      owner_present: helpMapMarkdown.includes(entry.owner),
    };
  }),
);
const helpMapOk = helpMapChecks.every(
  (entry) => entry.app_source_count >= 1 && entry.map_count === 1 && entry.owner_present,
);

await mkdir(outputDirectory, { recursive: true });
const packageMetadata = JSON.parse(
  await readFile(join(managedPlaywrightPath, "package.json"), "utf8"),
);
const require = createRequire(import.meta.url);
const { chromium } = require(managedPlaywrightPath);
const scratchRoot = await mkdtemp(join(tmpdir(), "commandagent-g1-gui-smoke-"));
const providerProbeBin = providerOnly ? await createProviderProbeBinary() : null;

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
        await (providerOnly
          ? runProviderCase(smokeCase, providerProbeBin)
          : wizardOnly
          ? runWizardCase(smokeCase)
          : feedbackOnly
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
    mode: providerOnly
      ? "provider_propagation"
      : wizardOnly
        ? "pack_wizard"
        : readOnly
          ? "read_only"
          : overviewOnly
            ? "overview_only"
            : gateOneOnly
              ? "gate_one_only"
              : feedbackOnly
                ? "feedback_only"
                : pollingOnly
                  ? "polling_only"
                  : "full_trial",
    commandagent_bin: providerProbeBin ?? commandagentBin,
    provider: providerOnly ? "openai+gemini" : "ollama",
    model,
    fixture: fixtureRoot,
    scratch_runtime:
      results.length === cases.length && results.every((result) => result.ok)
        ? "removed_after_success"
        : scratchRoot,
  },
  help_map: {
    checks: helpMapChecks,
    path: "docs/user/gui-help-map.md",
    ok: helpMapOk,
  },
  cases: results,
  ok: helpMapOk && results.every((result) => result.ok),
};
await writeFile(join(outputDirectory, "browser-smoke.json"), `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify(report, null, 2));
if (!report.ok) process.exitCode = 1;

async function createProviderProbeBinary() {
  const path = join(scratchRoot, "provider-probe-commandagent");
  const terminalEvents = await readFile(
    join(repositoryRoot, "tests/fixtures/gui_cli_events.jsonl"),
    "utf8",
  );
  const script = [
    "#!/bin/sh",
    'if [ "${1-}" = "--version" ]; then',
    "  printf 'commandagent 0.1.0 provider-probe\\n'",
    "  exit 0",
    "fi",
    'run_directory=${COMMANDAGENT_EVAL_EVENTS%/*}',
    'printf \'%s\\n\' "$@" > "$run_directory/delegated-args.txt"',
    `printf '%s' '${terminalEvents.replaceAll("'", "'\\''")}' > "$COMMANDAGENT_EVAL_EVENTS"`,
    "",
  ].join("\n");
  await writeFile(path, script, { mode: 0o700 });
  await chmod(path, 0o700);
  return path;
}

async function runProviderCase(smokeCase, probeBinary) {
  const executionRoot = join(scratchRoot, `${smokeCase.id}-provider-execution`);
  await mkdir(executionRoot, { recursive: true });
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const server = await startServer(smokeCase.serverBasePath, executionRoot, probeBinary);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1050 } });
  const consoleErrors = [];
  page.on("console", (entry) => {
    if (entry.type() === "error") consoleErrors.push(entry.text());
  });
  try {
    const prefix = displayBasePath(smokeCase.serverBasePath);
    const trialUrl = new URL(`${prefix}try/`, server.origin).href;
    const response = await page.goto(trialUrl, { waitUntil: "networkidle" });
    await page
      .locator("[data-testid='trial-provider'] option[value='gemini']")
      .waitFor({ state: "attached" });
    const providers = [];
    for (const provider of ["openai", "gemini"]) {
      const executorModel = `${provider}-executor-model`;
      const plannerModel = `${provider}-planner-model`;
      await page.locator("[data-testid='trial-goal']").fill("Create a CLI --pattern filter command");
      await page.locator("[data-testid='trial-token']").fill(trialCredential);
      await page.locator("[data-testid='trial-provider']").selectOption(provider);
      await page.locator("[data-testid='trial-executor-model']").fill(executorModel);
      await page.locator("[data-testid='trial-planner-model']").fill(plannerModel);
      await page.locator("[data-testid='check-contract']").click();
      await page.locator("[data-testid='gate-one-card']").waitFor();
      await page.locator("[data-testid='gate-one-confirm']").check();
      const createRequestPromise = page.waitForRequest((candidate) => {
        const url = new URL(candidate.url());
        return candidate.method() === "POST" && url.pathname.endsWith("/api/sessions");
      });
      const createResponsePromise = page.waitForResponse((candidate) => {
        const url = new URL(candidate.url());
        return candidate.request().method() === "POST" && url.pathname.endsWith("/api/sessions");
      });
      await page.locator("[data-testid='launch-session']").click();
      const [createRequest, createResponse] = await Promise.all([
        createRequestPromise,
        createResponsePromise,
      ]);
      const createBody = createRequest.postDataJSON();
      const created = await createResponse.json();
      const delegatedArgs = (
        await readEventually(
          join(executionRoot, ".anvil", "runs", created.id, "delegated-args.txt"),
        )
      ).trim().split("\n");
      await page.locator("[data-testid='terminal-gate']").waitFor({ timeout: 10_000 });
      const identityText = await page.locator("[data-testid='trial-run-identity']").innerText();
      const result = {
        provider,
        request_provider: createBody.provider,
        request_planner_provider: createBody.planner_provider,
        cli_provider: cliArgumentValue(delegatedArgs, "--provider"),
        cli_planner_provider: cliArgumentValue(delegatedArgs, "--planner-provider"),
        cli_model: cliArgumentValue(delegatedArgs, "--model"),
        cli_planner_model: cliArgumentValue(delegatedArgs, "--planner-model"),
        identity_text: identityText,
      };
      result.ok =
        createResponse.status() === 202 &&
        result.request_provider === provider &&
        result.request_planner_provider === provider &&
        result.cli_provider === provider &&
        result.cli_planner_provider === provider &&
        result.cli_model === executorModel &&
        result.cli_planner_model === plannerModel &&
        identityText.includes(provider) &&
        identityText.includes(executorModel) &&
        identityText.includes(plannerModel);
      providers.push(result);
      await page
        .locator("[data-testid='runtime-status'][data-session-state='idle']")
        .waitFor({ timeout: 10_000 });
      if (provider !== "gemini") {
        await page.locator("[data-testid='close-session']").click();
        await page.locator("[data-testid='start-new-run']").click();
      }
    }
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-provider-propagation.png`),
    });
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      status: response?.status() ?? 0,
      providers,
      unexpected_console_errors: consoleErrors,
      ok:
        response?.status() === 200 &&
        providers.length === 2 &&
        providers.every((provider) => provider.ok) &&
        consoleErrors.length === 0,
    };
  } finally {
    await browser.close();
    server.stop();
  }
}

async function runWizardCase(smokeCase) {
  const executionRoot = join(scratchRoot, `${smokeCase.id}-wizard-execution`);
  await mkdir(executionRoot, { recursive: true });
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const server = await startServer(smokeCase.serverBasePath, executionRoot);
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1050 } });
  const consoleErrors = [];
  page.on("console", (entry) => {
    if (entry.type() === "error") consoleErrors.push(entry.text());
  });
  try {
    const wizard = await probePackWizard(
      page,
      browser,
      server.origin,
      smokeCase.serverBasePath,
    );
    const expectedFailureConsoleErrors = consoleErrors.filter((entry) =>
      entry.includes("server responded with a status of 422"),
    );
    const unexpectedConsoleErrors = consoleErrors.filter(
      (entry) => !entry.includes("server responded with a status of 422"),
    );
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      pack_wizard: wizard,
      expected_verification_failure_console_errors: expectedFailureConsoleErrors,
      unexpected_console_errors: unexpectedConsoleErrors,
      ok:
        wizard.ok &&
        expectedFailureConsoleErrors.length === 1 &&
        unexpectedConsoleErrors.length === 0,
    };
  } finally {
    await browser.close();
    server.stop();
  }
}

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
        JSON.stringify([
          "01\n概要",
          "02\nトライアル",
          "03\n拡張",
          "04\nリポジトリ実行記録",
          "05\n計測",
        ]) &&
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

    const gettingStarted = page.locator("[data-testid='getting-started']");
    await gettingStarted.waitFor();
    const gettingStartedText = await gettingStarted.textContent();
    const dashboardHelpCopy = helpMapEntries
      .filter((entry) => entry.source === "gui/components/getting-started.tsx")
      .every((entry) => gettingStartedText?.includes(entry.copy));
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-getting-started.png`),
    });
    const prerequisiteStatuses = await page
      .locator("[data-testid='getting-started-prerequisites'] .prerequisite-row")
      .evaluateAll((rows) => rows.map((row) => row.getAttribute("data-status")));
    await page.locator("[data-testid='getting-started-sample']").click();
    await page.locator("[data-testid='gate-one-primer']").waitFor();
    await page
      .locator("[data-testid='trial-pack'] option[value='cli-assist@1.0.0']")
      .waitFor({ state: "attached" });
    const samplePreset = {
      goal: await page.locator("[data-testid='trial-goal']").inputValue(),
      profile: await page.locator("[data-testid='trial-profile']").inputValue(),
      pack: await page.locator("[data-testid='trial-pack']").inputValue(),
      primer: await page.locator("[data-testid='gate-one-primer']").innerText(),
    };
    const samplePresetOk =
      samplePreset.goal === "Create a CLI --pattern filter command" &&
      samplePreset.profile === "python-cli" &&
      samplePreset.pack === "cli-assist@1.0.0" &&
      samplePreset.primer.includes("Gate 1 は CLI 実行前の確認です");
    await page.goBack({ waitUntil: "networkidle" });
    await gettingStarted.waitFor();
    await page.locator("[data-testid='getting-started-close']").click();
    await page.reload({ waitUntil: "networkidle" });
    await page.locator("[data-testid='runtime-status']").waitFor();
    await page.waitForTimeout(100);
    const dismissalPersistsInTab = (await gettingStarted.count()) === 0;
    const gettingStartedOk =
      prerequisiteStatuses.length === 3 &&
      prerequisiteStatuses.every((status) => status === "ready" || status === "action_required") &&
      dashboardHelpCopy &&
      samplePresetOk &&
      dismissalPersistsInTab;
    dashboard.getting_started = {
      mapped_help_copy_visible: dashboardHelpCopy,
      prerequisite_statuses: prerequisiteStatuses,
      sample_preset: samplePreset,
      dismissal_persists_in_tab: dismissalPersistsInTab,
    };

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
        ok: dashboardOk && dashboardAccessible && gettingStartedOk && consoleErrors.length === 0,
      };
    }
    const assets = await probePage(
      page,
      server.origin,
      smokeCase.serverBasePath,
      "assets/",
      "拡張",
      "拡張 | CommandAgent",
    );
    const extensionCatalog = await probeExtensionCatalog(page);
    const readOnlyUi = await probeReadOnlyUi(
      page,
      server.origin,
      smokeCase.serverBasePath,
      runIndex,
      smokeCase.id,
    );
    const measurements = readOnlyUi.pages.measurements;
    const runDetail = readOnlyUi.pages.run_detail;
    const readOnlyOk =
      dashboardOk &&
      dashboardAccessible &&
      gettingStartedOk &&
      assets.status === 200 &&
      assets.headingMatches &&
      assets.titleMatches &&
      extensionCatalog.ok &&
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
        pages: { assets, extension_catalog: extensionCatalog, measurements, run_detail: runDetail },
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
    const gateOneHashLayoutDesktop = await probeGateOneHashLayout(page, 1440);
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
    const gateOneHashLayoutMobile = await probeGateOneHashLayout(page, 390);
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-1-mobile.png`),
    });

    if (gateOneOnly) {
      const expectedNegativeConsoleErrors = consoleErrors.filter(
        (entry) =>
          entry === "Failed to load resource: the server responded with a status of 428 (Precondition Required)",
      );
      const unexpectedConsoleErrors = consoleErrors.filter(
        (entry) => !expectedNegativeConsoleErrors.includes(entry),
      );
      return {
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        gate_1: {
          card_markdown_visible_text: cardMarkdownText,
          copy_is_plain_japanese: gateOneCopyIsPlain,
          hash_layout: {
            desktop_1440: gateOneHashLayoutDesktop,
            mobile_390: gateOneHashLayoutMobile,
          },
          visible_text: gateOneText,
        },
        elapsed_seconds: (Date.now() - startedAt) / 1000,
        expected_negative_console_errors: expectedNegativeConsoleErrors,
        unexpected_console_errors: unexpectedConsoleErrors,
        ok:
          trialResponse?.status() === 200 &&
          trialTitle === "トライアル | CommandAgent" &&
          launchDisabledBeforeConfirmation &&
          gateOneCopyIsPlain &&
          gateOneHashLayoutDesktop.ok &&
          gateOneHashLayoutMobile.ok &&
          gateOneLayout.ok &&
          deniedWithoutConfirmation.status === 428 &&
          expectedNegativeConsoleErrors.length === 1 &&
          unexpectedConsoleErrors.length === 0,
      };
    }

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
    const newRunIdentityEditable = await allEnabled(launchIdentityControls, 7);
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
      extensionCatalog.ok &&
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
      gateOneHashLayoutDesktop.ok &&
      gateOneHashLayoutMobile.ok &&
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
      pages: { assets, extension_catalog: extensionCatalog, measurements, run_detail: runDetail, trial: { status: trialResponse?.status() ?? 0, title: trialTitle } },
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
        hash_layout: {
          desktop_1440: gateOneHashLayoutDesktop,
          mobile_390: gateOneHashLayoutMobile,
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

async function probeReadOnlyUi(page, origin, basePath, runIndex, caseId) {
  const runSummaries = runIndex.runs;
  const measurements = await probePage(
    page,
    origin,
    basePath,
    "measurements/",
    "計測",
    "計測 | CommandAgent",
  );
  const reportButtons = page.locator(".report-list button");
  await page.waitForFunction(
    () => document.querySelectorAll(".report-list button").length > 1,
  );
  const selectedReportPath = await reportButtons.nth(1).locator("small").innerText();
  await reportButtons.nth(1).click();
  await page.waitForFunction(
    (path) => document.querySelector(".report-list button.active small")?.textContent === path,
    selectedReportPath,
  );
  const reportRevalidated = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return response.request().method() === "GET" && url.pathname.endsWith("/api/reports");
  });
  await setDocumentVisibility(page, "hidden");
  await setDocumentVisibility(page, "visible");
  const reportResponse = await reportRevalidated;
  await reportResponse.finished();
  await page.waitForTimeout(100);
  const retainedReportPath = await page
    .locator(".report-list button.active small")
    .innerText();
  const selectionRetainedAfterVisibility = retainedReportPath === selectedReportPath;
  const measurementHeadings = await page.locator("main h1, main h2, main h3").evaluateAll(
    (headings) => headings.map((heading) => ({
      level: Number(heading.tagName.slice(1)),
      text: heading.textContent ?? "",
    })),
  );
  const measurementHeadingOrderValid = measurementHeadings.every(
    (heading, index) => index === 0 || heading.level <= measurementHeadings[index - 1].level + 1,
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
    "リポジトリ実行記録",
    "リポジトリ実行記録 | CommandAgent",
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
  const indexCountText = await page.locator("[data-testid='run-index-count']").innerText();
  const expectedIndexCountText = `表示件数 ${runSummaries.length} / 総数 ${runIndex.total}`;
  const indexedRunIds = new Set(runSummaries.map((run) => run.id));
  const repositoryRunEntries = await readdir(
    join(repositoryRoot, "workspace", "management", "runs"),
    { withFileTypes: true },
  );
  const omittedRunId = repositoryRunEntries
    .filter((entry) => entry.isDirectory() && !indexedRunIds.has(entry.name))
    .map((entry) => entry.name)
    .sort()
    .at(0);
  if (runIndex.total > runSummaries.length && omittedRunId === undefined) {
    throw new Error("Run index reports omitted runs but no omitted run directory was found");
  }
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
  let directLookup = {
    available: false,
    id: null,
    mobile_id_fits: true,
    opened: true,
    selected_id_visible: true,
  };
  if (omittedRunId !== undefined) {
    await filterInput.fill(omittedRunId);
    const directResponse = page.waitForResponse((response) => {
      const url = new URL(response.url());
      return response.request().method() === "GET" &&
        url.pathname.endsWith(`/api/runs/${encodeURIComponent(omittedRunId)}`);
    });
    await page.locator("[data-testid='run-direct-open']").click();
    await directResponse;
    await page.locator(".run-document .document-viewer").waitFor();
    const selectedId = page.locator("[data-testid='run-selected-id']");
    const selectedIdText = await selectedId.innerText();
    const selectedOptionValue = await page.locator("#run-select").inputValue();
    await page.setViewportSize({ width: 390, height: 844 });
    const mobileId = await selectedId.evaluate((output) => ({
      client_width: output.clientWidth,
      fits: output.scrollWidth <= output.clientWidth,
      scroll_width: output.scrollWidth,
    }));
    const mobileRunPageFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    directLookup = {
      available: true,
      id: omittedRunId,
      mobile_id_fits: mobileId.fits && mobileRunPageFits,
      opened: selectedOptionValue === omittedRunId,
      selected_id_visible: selectedIdText.includes(omittedRunId),
    };
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${caseId}-run-detail-mobile-id.png`),
    });
    await page.setViewportSize({ width: 1440, height: 1050 });
  }
  await page.screenshot({
    fullPage: true,
    path: join(outputDirectory, `${caseId}-run-detail.png`),
  });
  const requestOwnership = await probeRunSelectionOwnership(page, runSummaries);

  const fullSizeLinkPresent = fullSizeHref?.endsWith("/api/maps/score-time.svg") ?? false;
  const unselectedHasNoRecords = !unselectedText.includes("NO RECORDS");
  return {
    pages: { measurements, run_detail: runDetail },
    measurement_selection: {
      after_visibility: retainedReportPath,
      before_visibility: selectedReportPath,
      heading_order_valid: measurementHeadingOrderValid,
      headings: measurementHeadings,
      selection_retained_after_visibility: selectionRetainedAfterVisibility,
    },
    run_selection: {
      count: indexCountText,
      count_matches_index_total: indexCountText === expectedIndexCountText,
      direct_lookup: directLookup,
      displayed_options: displayedOptions.length,
      expected_options: expectedOptions.length,
      filter_matches_id: filterMatchesId,
      no_records_label_absent: unselectedHasNoRecords,
      no_match_label_visible: noMatchLabelVisible,
      options_include_dates_and_status: optionsIncludeDatesAndStatus,
      request_ownership: requestOwnership,
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
      selectionRetainedAfterVisibility &&
      measurementHeadingOrderValid &&
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      runDetail.titleMatches &&
      unselectedHasNoRecords &&
      noMatchLabelVisible &&
      filterMatchesId &&
      indexCountText === expectedIndexCountText &&
      directLookup.opened &&
      directLookup.selected_id_visible &&
      directLookup.mobile_id_fits &&
      optionsIncludeDatesAndStatus &&
      requestOwnership.ok &&
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

async function probeRunSelectionOwnership(page, runSummaries) {
  const runIds = runSummaries.slice(0, 3).map((run) => run.id);
  if (runIds.length < 3) {
    throw new Error("Run selection ownership probe requires at least three runs");
  }
  const [, supersededRunId, currentRunId] = runIds;
  const supersededRelease = deferred();
  const currentRelease = deferred();
  const supersededStarted = deferred();
  const currentStarted = deferred();
  const releases = new Map([
    [supersededRunId, supersededRelease],
    [currentRunId, currentRelease],
  ]);
  const starts = new Map([
    [supersededRunId, supersededStarted],
    [currentRunId, currentStarted],
  ]);
  const routePattern = "**/api/runs/*";
  const routeHandler = async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    const marker = "/api/runs/";
    const markerIndex = pathname.lastIndexOf(marker);
    const encodedId = markerIndex === -1 ? "" : pathname.slice(markerIndex + marker.length);
    if (encodedId.includes("/")) {
      await route.continue();
      return;
    }
    const id = decodeURIComponent(encodedId);
    const release = releases.get(id);
    if (release === undefined) {
      await route.continue();
      return;
    }
    starts.get(id)?.resolve();
    await release.promise;
    await route.fulfill({
      contentType: "application/json",
      status: 200,
      body: JSON.stringify({
        id,
        acceptance_path: "acceptance.md",
        acceptance: `# RUN SWITCH ${id}`,
        evidence: [{ id: "evidence.log", path: "evidence.log", size_bytes: 1 }],
      }),
    }).catch(() => undefined);
  };

  await page.route(routePattern, routeHandler);
  try {
    await page.locator("#run-filter").fill("");
    await page.locator("#run-select").selectOption(supersededRunId);
    await supersededStarted.promise;
    await page.locator("#run-select").selectOption(currentRunId);
    await currentStarted.promise;
    await page.waitForTimeout(50);

    const pendingSelection = {
      evidence_list_count: await page.locator(".evidence-list").count(),
      loading_visible: await page.locator(".run-document [role='status']").isVisible(),
      viewer_count: await page.locator(".run-document .document-viewer").count(),
    };

    currentRelease.resolve();
    await page.waitForFunction(
      (markerText) =>
        document.querySelector("[data-testid='document-content']")?.textContent?.includes(markerText),
      `RUN SWITCH ${currentRunId}`,
    );
    const currentContent = await page.locator("[data-testid='document-content']").innerText();

    await page.locator("#run-select").selectOption("");
    await page.waitForFunction(
      () =>
        [...document.querySelectorAll(".run-document .state-code")]
          .some((node) => node.textContent === "実行未選択"),
    );
    const emptySelection = {
      empty_selection_cleared:
        (await page.locator(".evidence-list").count()) === 0 &&
        (await page.locator(".run-document .document-viewer").count()) === 0,
      evidence_list_count: await page.locator(".evidence-list").count(),
      viewer_count: await page.locator(".run-document .document-viewer").count(),
    };

    const currentRunWon =
      currentContent.includes(`RUN SWITCH ${currentRunId}`) &&
      !currentContent.includes(`RUN SWITCH ${supersededRunId}`);
    return {
      current_run_won: currentRunWon,
      empty_selection: emptySelection,
      pending_selection: pendingSelection,
      ok:
        pendingSelection.loading_visible &&
        pendingSelection.viewer_count === 0 &&
        pendingSelection.evidence_list_count === 0 &&
        currentRunWon &&
        emptySelection.empty_selection_cleared,
    };
  } finally {
    supersededRelease.resolve();
    currentRelease.resolve();
    await page.unroute(routePattern, routeHandler);
  }
}

async function probeTrialFeedback(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const sessionId = "0198b9c8-fab8-7000-8000-000000000069";
  const startedEpochSeconds = Date.parse("2026-08-16T00:00:00Z") / 1_000;
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
            started_epoch_seconds: startedEpochSeconds,
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
          body: JSON.stringify(
            syntheticFeedbackSession(sessionId, phaseTotal, terminal, startedEpochSeconds),
          ),
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
    const gateTwoIdentity = await readTrialRunIdentity(page);
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

    const sessionQueryBeforeReload = new URL(page.url()).searchParams.get("session");
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(
      ({ expectedId, expectedToken }) => {
        const token = document.querySelector("[data-testid='trial-token']");
        const reconnect = document.querySelector("[data-testid='reconnect-session-button']");
        return (
          new URL(window.location.href).searchParams.get("session") === expectedId &&
          token instanceof HTMLInputElement &&
          token.value === expectedToken &&
          reconnect instanceof HTMLButtonElement &&
          !reconnect.disabled
        );
      },
      { expectedId: sessionId, expectedToken: "synthetic-feedback-token" },
    );
    await page.locator("[data-testid='reconnect-session-button']").click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const reconnectedIdentity = await readTrialRunIdentity(page);
    const reconnectedElapsed = page.locator("[data-testid='elapsed-time']");
    const elapsedAfterReconnect = Number(
      await reconnectedElapsed.getAttribute("data-elapsed-seconds"),
    );
    const meanAfterReconnect = await page
      .locator("[data-testid='mean-duration-comparison'] strong")
      .innerText();
    const elapsedPreservedAfterReconnect =
      sessionQueryBeforeReload === sessionId && elapsedAfterReconnect >= elapsedAfter;
    const meanPreservedAfterReconnect = meanAfterReconnect === meanText;

    terminal = true;
    await page.clock.runFor(1_100);
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const terminalIdentity = await readTrialRunIdentity(page);
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
      elapsed_after_reconnect_seconds: elapsedAfterReconnect,
      elapsed_preserved_after_reconnect: elapsedPreservedAfterReconnect,
      zero_total_hidden: zeroTotalHidden,
      phase_text: phaseText,
      phase_uses_total: phaseText === "フェーズ 2 / 5",
      measured_mean_text: meanText,
      measured_mean_visible: meanText === "平均 10.2 分",
      measured_mean_after_reconnect: meanAfterReconnect,
      mean_preserved_after_reconnect: meanPreservedAfterReconnect,
      mean_is_not_eta: meanLabel.includes("予測ではありません"),
      monitor_and_progress_separate: feedbackAfterMonitor === 1,
      gate_2_identity: gateTwoIdentity,
      reconnected_identity: reconnectedIdentity,
      terminal_identity: terminalIdentity,
      running_title: runningTitle,
      terminal_title: terminalTitle,
      title_changed: titleChanged,
      ok:
        elapsedChanged &&
        elapsedPreservedAfterReconnect &&
        meanPreservedAfterReconnect &&
        zeroTotalHidden &&
        phaseText === "フェーズ 2 / 5" &&
        meanText === "平均 10.2 分" &&
        meanLabel.includes("予測ではありません") &&
        feedbackAfterMonitor === 1 &&
        syntheticFeedbackIdentityMatches(gateTwoIdentity) &&
        syntheticFeedbackIdentityMatches(reconnectedIdentity) &&
        syntheticFeedbackIdentityMatches(terminalIdentity) &&
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
                pack: {
                  id: "cli-assist",
                  version: "1.0.0",
                  hash: "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd",
                  source: "admitted",
                  source_label: "承認済み",
                },
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
        sessionText.includes("cli-assist@1.0.0 · 承認済み") &&
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
            started_epoch_seconds: Date.parse("2026-08-16T00:00:00Z") / 1_000,
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
              started_epoch_seconds: Date.parse("2026-08-16T00:00:00Z") / 1_000,
              average_duration_seconds: null,
              gate: "gate_2",
              status: "running",
              verdict: null,
              assurance: null,
              phases: [],
              event_count: 0,
              acceptance_sheet: null,
              section5: null,
              events_path: `.anvil/runs/${sessionId}/events.jsonl`,
              identity: syntheticProposal().identity,
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
      pack: {
        selection: "pinned",
        id: "cli-assist",
        version: "1.0.0",
        hash: `sha256:${"9".repeat(64)}`,
        point: "cli-validation",
        source: "admitted",
      },
    },
    price: {
      ...proposal.price,
      duration_n: 5,
      average_duration_seconds: 612,
    },
  };
}

function syntheticFeedbackSession(sessionId, phaseTotal, terminal, startedEpochSeconds) {
  return {
    id: sessionId,
    started_epoch_seconds: startedEpochSeconds,
    average_duration_seconds: 612,
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
    identity: syntheticFeedbackProposal().identity,
  };
}

async function readTrialRunIdentity(page) {
  const identity = page.locator("[data-testid='trial-run-identity']");
  await identity.waitFor();
  return {
    goal: await identity.locator("[data-testid='trial-run-identity-goal']").innerText(),
    profile: await identity.locator("[data-testid='trial-run-identity-profile']").innerText(),
    pack: await identity.locator("[data-testid='trial-run-identity-pack']").innerText(),
    executor_model: await identity
      .locator("[data-testid='trial-run-identity-executor-model']")
      .innerText(),
    planner_model: await identity
      .locator("[data-testid='trial-run-identity-planner-model']")
      .innerText(),
  };
}

function syntheticFeedbackIdentityMatches(identity) {
  return (
    identity.goal === "Synthetic Gate 2 feedback probe" &&
    identity.profile === "python-cli" &&
    identity.pack === "cli-assist@1.0.0" &&
    identity.executor_model === "ollama / synthetic-model" &&
    identity.planner_model === "ollama / synthetic-model"
  );
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

async function probeGateOneHashLayout(page, expectedViewportWidth) {
  const elements = await page
    .locator("[data-testid='gate-one-card-markdown'], .confirmation-id .hash-line")
    .evaluateAll((targets) =>
      targets.map((target) => {
        const hashes = target.textContent?.match(/sha256:[0-9a-f]+/g) ?? [];
        return {
          class_name: target.getAttribute("class") ?? "",
          client_width: target.clientWidth,
          hash_count: hashes.length,
          scroll_width: target.scrollWidth,
          scroll_width_within_client: target.scrollWidth <= target.clientWidth,
          test_id: target.getAttribute("data-testid"),
        };
      }),
    );
  const viewportWidth = page.viewportSize()?.width ?? 0;
  return {
    elements,
    expected_viewport_width: expectedViewportWidth,
    ok:
      viewportWidth === expectedViewportWidth &&
      elements.length === 2 &&
      elements.every(
        (element) =>
          element.client_width > 0 &&
          element.hash_count > 0 &&
          element.scroll_width_within_client,
      ),
    viewport_width: viewportWidth,
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
      pack: { selection: "none" },
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

async function setDocumentVisibility(page, visibilityState) {
  await page.evaluate((state) => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => state,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  }, visibilityState);
}

async function probePackWizard(page, browser, origin, basePath) {
  const prefix = displayBasePath(basePath);
  const assetsUrl = new URL(`${prefix}assets/`, origin).href;
  await page.goto(assetsUrl, { waitUntil: "networkidle" });
  await page.evaluate(
    ({ key, token }) => window.sessionStorage.setItem(key, token),
    { key: trialTokenStorageKey(basePath), token: trialCredential },
  );
  await page.reload({ waitUntil: "networkidle" });
  await page.locator("[data-testid='pack-wizard-open']").click();
  await page.locator("[data-testid='pack-wizard-target']").waitFor();
  await page.getByRole("button", { name: "出発点を選ぶ" }).click();
  await page.locator("[data-testid='pack-wizard-nextjs-acme']").check();
  await page.getByRole("button", { name: "編集を開始" }).click();

  const assist = page.locator("[data-testid='pack-wizard-assist']");
  const validAssist = await assist.inputValue();
  await assist.fill(`${validAssist}unexpected: true\n`);
  await page.getByRole("button", { name: "保存して検証" }).click();
  const issueList = page.locator("[data-testid='pack-wizard-issues']");
  await issueList.waitFor();
  const issueText = await issueList.innerText();
  const focusAction = issueList.getByRole("button", { name: "該当項目へ移動" }).first();
  const focusTarget = await focusAction.getAttribute("data-focus-target");
  await focusAction.click();
  await page.waitForFunction(() => document.activeElement?.id === "pack-wizard-assist");
  const activeAfterFailure = await page.evaluate(() => document.activeElement?.id ?? null);

  await assist.fill(validAssist);
  await page.getByRole("button", { name: "保存して検証" }).click();
  const success = page.locator("[data-testid='pack-verification-success']");
  await success.waitFor();
  const verifiedHash = await page.locator("[data-testid='pack-wizard-hash']").innerText();

  await page.getByRole("button", { name: "編集に戻る" }).click();
  const editedAssist = `${validAssist}# unsaved Issue 165 edit\n`;
  await assist.fill(editedAssist);
  await page.locator(".pack-wizard-steps li:nth-child(4) button").click();
  await page.getByRole("button", { name: "保存済み bytes を再検証" }).click();
  await success.waitFor();
  const reverifiedHash = await page.locator("[data-testid='pack-wizard-hash']").innerText();
  await page.getByRole("button", { name: "編集に戻る" }).click();
  const displayedMembers = await readWizardMembers(page);
  const displayedBytesReconciled =
    displayedMembers["assist.yaml"] === validAssist &&
    displayedMembers["assist.yaml"] !== editedAssist;
  await page.locator(".pack-wizard-steps li:nth-child(4) button").click();
  await page.locator("[data-testid='pack-wizard-to-pin']").click();
  await page.locator("[data-testid='pack-wizard-pin-action']").click();
  await page.locator("[data-testid='pack-wizard-pinned']").waitFor();
  const pinnedDetail = await page.evaluate(
    async ({ packUrl, token }) => {
      const response = await fetch(packUrl, {
        headers: { "x-commandagent-trial-authorization": `Bearer ${token}` },
      });
      if (!response.ok) throw new Error(`pinned pack detail returned ${response.status}`);
      return response.json();
    },
    {
      packUrl: new URL(
        `${prefix}api/extensions/packs/nextjs-acme/1.0.0`,
        origin,
      ).href,
      token: trialCredential,
    },
  );
  const pinnedBytesMatchDisplay = memberMapsEqual(pinnedDetail.files, displayedMembers);
  const trialLink = page.locator("[data-testid='pack-wizard-trial-link']");
  const trialHref = await trialLink.getAttribute("href");
  if (trialHref === null) throw new Error("pinned wizard has no Trial handoff");
  const selector = new URL(trialHref, page.url()).searchParams.get("pack");

  await page.locator(".pack-wizard-steps li:nth-child(1) button").click();
  const pinnedTargetDisabled =
    (await page.locator("[data-testid='pack-wizard-profile']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard-intent']").isDisabled());
  await page.locator(".pack-wizard-steps li:nth-child(2) button").click();
  const pinnedStartingPointDisabled = await page
    .locator("input[name='pack-starting-point']")
    .first()
    .isDisabled();
  await page.locator(".pack-wizard-steps li:nth-child(3) button").click();
  const pinnedEditorDisabled =
    pinnedTargetDisabled &&
    pinnedStartingPointDisabled &&
    (await page.locator("[data-testid='pack-wizard-id']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard-version']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard-assist']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard-eval']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard-material'] textarea").first().isDisabled());
  await page.locator(".pack-wizard-steps li:nth-child(5) button").click();

  const trialPage = await browser.newPage({ viewport: { width: 1200, height: 900 } });
  let selectedPack = null;
  try {
    const trialResponse = await trialPage.goto(new URL(trialHref, page.url()).href, {
      waitUntil: "networkidle",
    });
    await trialPage.locator("[data-testid='trial-pack']").waitFor();
    selectedPack = await trialPage.locator("[data-testid='trial-pack']").inputValue();
    if (trialResponse?.status() !== 200) {
      throw new Error(`wizard Trial handoff returned ${trialResponse?.status()}`);
    }
  } finally {
    await trialPage.close();
  }

  await page.locator("[data-testid='pack-wizard-new-version']").click();
  await page.locator("[data-testid='pack-wizard-editor']").waitFor();
  const pinnedNextVersion = await page.locator("[data-testid='pack-wizard-version']").inputValue();
  const pinnedNextDraftEditable =
    !(await page.locator("[data-testid='pack-wizard-id']").isDisabled()) &&
    !(await page.locator("[data-testid='pack-wizard-version']").isDisabled()) &&
    !(await page.locator("[data-testid='pack-wizard-assist']").isDisabled());
  const pinnedNextMembers = await readWizardMembers(page);
  const pinnedNextMembersCopied = memberMapsMatchNextVersion(
    displayedMembers,
    pinnedNextMembers,
    "1.0.0",
    pinnedNextVersion,
  );
  await page.getByRole("button", { name: "保存して検証" }).click();
  await success.waitFor();
  const stagedNextDetail = await page.evaluate(
    async ({ packUrl, token }) => {
      const response = await fetch(packUrl, {
        headers: { "x-commandagent-trial-authorization": `Bearer ${token}` },
      });
      if (!response.ok) throw new Error(`next pack detail returned ${response.status}`);
      return response.json();
    },
    {
      packUrl: new URL(
        `${prefix}api/extensions/packs/nextjs-acme/${pinnedNextVersion}`,
        origin,
      ).href,
      token: trialCredential,
    },
  );
  const pinnedNextVersionStaged =
    (await page.locator("[data-testid='pack-wizard']").getAttribute("data-lifecycle")) === "staged" &&
    stagedNextDetail.version === pinnedNextVersion &&
    stagedNextDetail.report.status === "staged";
  await page.locator("[data-testid='pack-wizard-to-pin']").click();
  await page.locator("[data-testid='pack-wizard-pin-action']").click();
  await page.locator("[data-testid='pack-wizard-pinned']").waitFor();
  await page.locator(".pack-retire-panel summary").click();
  await page.locator("[data-testid='pack-wizard-retire-confirm']").check();
  await page.locator("[data-testid='pack-wizard-retire-action']").click();
  await page.locator("[data-testid='pack-wizard-retired']").waitFor();
  const retiredHasNoTrialLink = (await page.locator("[data-testid='pack-wizard-trial-link']").count()) === 0;
  await page.locator(".pack-wizard-steps li:nth-child(3) button").click();
  const retiredEditorDisabled = await page.locator("[data-testid='pack-wizard-assist']").isDisabled();
  await page.locator(".pack-wizard-steps li:nth-child(5) button").click();
  await page.locator("[data-testid='pack-wizard-new-version']").click();
  await page.locator("[data-testid='pack-wizard-editor']").waitFor();
  const retiredNextVersion = await page.locator("[data-testid='pack-wizard-version']").inputValue();
  const retiredNextDraftEditable =
    retiredNextVersion === "1.0.2" &&
    !(await page.locator("[data-testid='pack-wizard-assist']").isDisabled()) &&
    (await page.locator("[data-testid='pack-wizard']").getAttribute("data-lifecycle")) === "draft";
  const retiredNextMembersCopied = memberMapsMatchNextVersion(
    pinnedNextMembers,
    await readWizardMembers(page),
    pinnedNextVersion,
    retiredNextVersion,
  );

  return {
    active_after_failure: activeAfterFailure,
    displayed_bytes_reconciled: displayedBytesReconciled,
    failure_focus_target: focusTarget,
    failure_text: issueText,
    pinned_bytes_match_display: pinnedBytesMatchDisplay,
    pinned_editor_disabled: pinnedEditorDisabled,
    pinned_next_draft_editable: pinnedNextDraftEditable,
    pinned_next_members_copied: pinnedNextMembersCopied,
    pinned_next_version: pinnedNextVersion,
    pinned_next_version_staged: pinnedNextVersionStaged,
    retired_editor_disabled: retiredEditorDisabled,
    retired_has_no_trial_link: retiredHasNoTrialLink,
    retired_next_draft_editable: retiredNextDraftEditable,
    retired_next_members_copied: retiredNextMembersCopied,
    retired_next_version: retiredNextVersion,
    selected_pack: selectedPack,
    selector,
    reverified_hash: reverifiedHash,
    verified_hash: verifiedHash,
    ok:
      issueText.includes("assist.yaml") &&
      focusTarget === "pack-wizard-assist" &&
      activeAfterFailure === "pack-wizard-assist" &&
      verifiedHash === expectedNextjsAcmeHash &&
      reverifiedHash === verifiedHash &&
      displayedBytesReconciled &&
      pinnedBytesMatchDisplay &&
      pinnedDetail.report.hash === reverifiedHash &&
      pinnedEditorDisabled &&
      pinnedNextVersion === "1.0.1" &&
      pinnedNextDraftEditable &&
      pinnedNextMembersCopied &&
      pinnedNextVersionStaged &&
      selector === "nextjs-acme@1.0.0" &&
      selectedPack === selector &&
      retiredEditorDisabled &&
      retiredHasNoTrialLink &&
      retiredNextDraftEditable &&
      retiredNextMembersCopied,
  };
}

async function readWizardMembers(page) {
  const members = {};
  const assist = await page.locator("[data-testid='pack-wizard-assist']").inputValue();
  const evalDocument = await page.locator("[data-testid='pack-wizard-eval']").inputValue();
  if (assist.trim() !== "") members["assist.yaml"] = assist;
  if (evalDocument.trim() !== "") members["eval.yaml"] = evalDocument;
  const materials = await page.locator("[data-testid='pack-wizard-material']").evaluateAll((rows) =>
    rows.map((row) => ({
      content: row.querySelector("textarea")?.value ?? "",
      name: row.querySelector("input")?.value ?? "",
    })),
  );
  for (const material of materials) members[`materials/${material.name}`] = material.content;
  return members;
}

function memberMapsEqual(left, right) {
  const entries = (members) => Object.entries(members).sort(([a], [b]) => a.localeCompare(b));
  return JSON.stringify(entries(left)) === JSON.stringify(entries(right));
}

function memberMapsMatchNextVersion(previous, next, previousVersion, nextVersion) {
  const replaceVersion = (document) =>
    document.replace(`  version: ${previousVersion}`, `  version: ${nextVersion}`);
  const expected = {
    ...previous,
    ...(previous["assist.yaml"] === undefined
      ? {}
      : { "assist.yaml": replaceVersion(previous["assist.yaml"]) }),
    ...(previous["eval.yaml"] === undefined
      ? {}
      : { "eval.yaml": replaceVersion(previous["eval.yaml"]) }),
  };
  return memberMapsEqual(expected, next);
}

async function probeExtensionCatalog(page) {
  const rows = page.locator("[data-testid='extension-pack-row']");
  await rows.first().waitFor();
  const rowCount = await rows.count();
  const sourceLabels = await rows.locator(".pack-source").allInnerTexts();
  const trialLink = rows.locator("[data-testid='pack-trial-link']").first();
  const trialLinkText = await trialLink.innerText();
  const href = await trialLink.getAttribute("href");
  if (href === null) throw new Error("extension catalog has no eligible Trial handoff");
  const selector = new URL(href, page.url()).searchParams.get("pack");
  const response = await page.goto(new URL(href, page.url()).href, { waitUntil: "networkidle" });
  const selectedPack = await page.locator("[data-testid='trial-pack']").inputValue();
  return {
    row_count: rowCount,
    selected_pack: selectedPack,
    selector,
    source_labels: sourceLabels,
    status: response?.status() ?? 0,
    ok:
      response?.status() === 200 &&
      sourceLabels.includes("承認済み") &&
      sourceLabels.includes("リポジトリ（未承認）") &&
      trialLinkText.includes("Trial で使う") &&
      selector !== null &&
      selectedPack === selector,
  };
}

async function startServer(basePath, executionRoot, delegateBin = commandagentBin) {
  const extensionRoot = `${executionRoot}-extensions`;
  await mkdir(extensionRoot, { recursive: true, mode: 0o700 });
  await chmod(extensionRoot, 0o700);
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
      "--extension-root",
      extensionRoot,
      "--trial-token-auth",
      "on",
      "--commandagent-bin",
      delegateBin,
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

async function readEventually(path) {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    try {
      return await readFile(path, "utf8");
    } catch (reason) {
      if (reason?.code !== "ENOENT") throw reason;
    }
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 50));
  }
  throw new Error(`timed out waiting for ${path}`);
}

function cliArgumentValue(arguments_, name) {
  const index = arguments_.indexOf(name);
  return index === -1 ? null : arguments_[index + 1] ?? null;
}

function displayBasePath(basePath) {
  return basePath === "/" ? "/" : `${basePath}/`;
}

function trialTokenStorageKey(basePath) {
  return `commandagent.gui.trial-token:${basePath}`;
}

function deferred() {
  let resolvePromise;
  const promise = new Promise((resolve) => {
    resolvePromise = resolve;
  });
  return { promise, resolve: resolvePromise };
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

function countOccurrences(haystack, needle) {
  return haystack.split(needle).length - 1;
}
