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
    "usage: npm run smoke -- --output <evidence-directory> [--read-only] [--commandagent-bin <path>] [--model <name>]",
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
    mode: readOnly ? "read_only" : "full_trial",
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
    const runSummaries = await page.evaluate(async () => {
      const mapSource =
        document.querySelector("[data-testid='score-time-map']")?.getAttribute("src") ?? "";
      const apiRoot = mapSource.replace(/maps\/score-time\.svg$/, "");
      const result = await fetch(`${apiRoot}runs`);
      const payload = await result.json();
      return Array.isArray(payload) ? payload : payload.runs;
    });

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
    const readOnlyUi = await probeReadOnlyUi(
      page,
      server.origin,
      smokeCase.serverBasePath,
      runSummaries,
      smokeCase.id,
    );
    const measurements = readOnlyUi.pages.measurements;
    const runDetail = readOnlyUi.pages.run_detail;
    const readOnlyOk =
      response?.status() === 200 &&
      heading === "Evidence, at a glance." &&
      map.complete &&
      map.naturalWidth > 0 &&
      apiChecks.every((check) => check.status === 200) &&
      linksUseBasePath &&
      assets.status === 200 &&
      assets.headingMatches &&
      readOnlyUi.ok &&
      consoleErrors.length === 0;

    if (readOnly) {
      return {
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        dashboard: { status: response?.status() ?? 0, heading },
        api_checks: apiChecks,
        svg: map,
        links_use_base_path: linksUseBasePath,
        pages: { assets, measurements, run_detail: runDetail },
        issue_75: readOnlyUi,
        elapsed_seconds: (Date.now() - startedAt) / 1000,
        unexpected_console_errors: consoleErrors,
        ok: readOnlyOk,
      };
    }

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

    await page.locator("[data-testid='gate-one-confirm']").check();
    await launch.click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const sessionId = await page.locator("[data-testid='session-progress'] h2").innerText();
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-2.png`),
    });
    await page.locator("[data-testid='terminal-gate']").waitFor({ timeout: trialTimeoutMs });
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
    await page.screenshot({
      fullPage: true,
      path: join(outputDirectory, `${smokeCase.id}-gate-terminal.png`),
    });

    const eventsPath = join(executionRoot, ".anvil", "runs", sessionId, "events.jsonl");
    const eventBytes = await readFile(eventsPath);
    await writeFile(join(outputDirectory, `${smokeCase.id}-events.jsonl`), eventBytes);
    const apiLog = {
      schema_version: "commandagent.gui-api-smoke/v1",
      base_path: smokeCase.buildBasePath,
      denied_without_confirmation: deniedWithoutConfirmation,
      observed_calls: apiCalls,
      terminal_poll: finalApi,
    };
    await writeFile(
      join(outputDirectory, `${smokeCase.id}-api-log.json`),
      `${JSON.stringify(apiLog, null, 2)}\n`,
    );
    const expectedNegativeConsoleErrors = consoleErrors.filter(
      (entry) =>
        entry === "Failed to load resource: the server responded with a status of 428 (Precondition Required)",
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
      assets.status === 200 &&
      assets.headingMatches &&
      measurements.status === 200 &&
      measurements.headingMatches &&
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      readOnlyUi.ok &&
      trialResponse?.status() === 200 &&
      launchDisabledBeforeConfirmation &&
      deniedWithoutConfirmation.status === 428 &&
      finalApi.status === 200 &&
      ["gate_3", "gate_4"].includes(finalApi.body.gate) &&
      expectedNegativeConsoleErrors.length === 1 &&
      unexpectedConsoleErrors.length === 0;
    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      dashboard: { status: response?.status() ?? 0, heading },
      api_checks: apiChecks,
      svg: map,
      links_use_base_path: linksUseBasePath,
      pages: { assets, measurements, run_detail: runDetail, trial: { status: trialResponse?.status() ?? 0 } },
      issue_75: readOnlyUi,
      gate_1: {
        launch_disabled_before_confirmation: launchDisabledBeforeConfirmation,
        api_without_confirmation_status: deniedWithoutConfirmation.status,
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
        terminal_visible_text: terminalText,
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

async function probeReadOnlyUi(page, origin, basePath, runSummaries, caseId) {
  const measurements = await probePage(
    page,
    origin,
    basePath,
    "measurements/",
    "Claims need coordinates.",
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
    "One run. Every receipt.",
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
        ? "time unavailable"
        : formatter.format(new Date(run.modified_epoch_seconds * 1000));
      return { text: `${date} — ${run.id}`, value: run.id };
    });
  }, runSummaries);
  const optionsIncludeDates = JSON.stringify(displayedOptions) === JSON.stringify(expectedOptions);
  const firstRunId = runSummaries[0]?.id ?? "";
  if (firstRunId === "") throw new Error("Run detail probe requires at least one run");
  await page.locator("#run-select").selectOption(firstRunId);
  await page.locator(".document-viewer").waitFor();

  const content = page.locator("[data-testid='document-content']");
  const toggle = page.locator("[data-testid='document-wrap-toggle']");
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
      no_records_label_absent: unselectedHasNoRecords,
      options_include_dates: optionsIncludeDates,
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
      runDetail.status === 200 &&
      runDetail.headingMatches &&
      unselectedHasNoRecords &&
      optionsIncludeDates &&
      wrapToggle.classes_switch &&
      initialPressed === "true" &&
      toggledPressed === "false" &&
      restoredPressed === "true" &&
      mobileMap.horizontally_scrollable &&
      mobilePageFits &&
      fullSizeLinkPresent,
  };
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
