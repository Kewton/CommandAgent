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
const rejectedTrialToken = `${trialToken}-wrong`;
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
      const resourceRevalidation = await probeResourceRevalidation(
        browser,
        server.origin,
        smokeCase.serverBasePath,
      );
      results.push({
        id: smokeCase.id,
        base_path: smokeCase.buildBasePath,
        lifecycle,
        resource_revalidation: resourceRevalidation,
        source_matrix: sourceMatrix,
        ok: lifecycle.ok && resourceRevalidation.ok && sourceMatrix.ok,
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
  let maxConcurrentRuntimeCalls = 0;
  const activeRuntimeRequests = new Set();
  let runtimeSession = null;
  let failNextIndex = false;
  let terminalReleased = false;
  let releaseTerminal;
  const terminalReady = new Promise((resolveTerminal) => {
    releaseTerminal = resolveTerminal;
  });
  const sessionRequests = [];

  page.on("request", (request) => {
    if (!isRuntimeStatusRequest(request)) return;
    activeRuntimeRequests.add(request);
    maxConcurrentRuntimeCalls = Math.max(
      maxConcurrentRuntimeCalls,
      activeRuntimeRequests.size,
    );
  });
  const settleRuntimeRequest = (request) => activeRuntimeRequests.delete(request);
  page.on("requestfailed", settleRuntimeRequest);
  page.on("requestfinished", settleRuntimeRequest);

  try {
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      const method = request.method();
      const authorization = request.headers()["x-commandagent-trial-authorization"];
      if (pathname.endsWith("/api/runtime-status") && method === "GET") {
        runtimeCalls += 1;
        await delay(75);
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
      if (
        pathname.includes("/api/sessions") &&
        authorization === `Bearer ${rejectedTrialToken}`
      ) {
        sessionRequests.push({ method, pathname });
        await json(route, 401, {
          code: "trial_token_invalid",
          error: "synthetic rejected Trial token",
        });
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
          started_epoch_seconds: 1_723_769_600,
          gate: "gate_2",
          status: "starting",
          events_path: `.commandagent/runs/${createdSessionId}/events.jsonl`,
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
    const runtimeLiveRegion = await page.locator("[data-testid='runtime-status']").evaluate(
      (node) => ({
        aria_atomic: node.getAttribute("aria-atomic"),
        aria_live: node.getAttribute("aria-live"),
      }),
    );
    const runtimeLiveRegionIsPoliteAtomic =
      runtimeLiveRegion.aria_live === "polite" && runtimeLiveRegion.aria_atomic === "true";
    const authPending = await page.locator("[data-testid='trial-session-auth-required']").innerText();
    assertIncludes(authPending, "認証待ち", "unauthenticated Trial history state");
    assert(indexCalls === 0, `unauthenticated page issued ${indexCalls} index requests`);

    await page.locator("[data-testid='trial-token']").fill(trialToken);
    await waitFor(() => indexCalls >= 1, "initial token index refresh");
    await page.locator("[data-testid='trial-session-freshness']").waitFor();
    await waitFor(
      () => activeRuntimeRequests.size === 0,
      "runtime request settlement before hiding",
    );
    await setDocumentVisibility(page, "hidden");
    await delay(150);
    const hiddenRuntimeCalls = runtimeCalls;
    await delay(3_350);
    const runtimePausedWhileHidden = runtimeCalls === hiddenRuntimeCalls;
    const indexCallsBeforeVisible = indexCalls;
    await setDocumentVisibility(page, "visible");
    await waitFor(() => runtimeCalls > hiddenRuntimeCalls, "visible runtime refresh");
    await waitFor(() => indexCalls > indexCallsBeforeVisible, "visible index refresh");
    const runtimeResumedWhenVisible = runtimeCalls > hiddenRuntimeCalls;
    await waitFor(
      () => activeRuntimeRequests.size === 0,
      "visible runtime refresh settlement",
    );
    const initialIndexCalls = indexCalls;

    runtimeSession = { id: createdSessionId, state: "running" };
    await page
      .locator("[data-testid='runtime-status'][data-session-state='running']")
      .waitFor({ timeout: 10_000 });
    await delay(1_000);
    const noPeriodicIndexPolling = indexCalls === initialIndexCalls && runtimeCalls >= 2;
    runtimeSession = null;
    const terminalRefreshStartedAt = Date.now();
    await page
      .locator("[data-testid='runtime-status'][data-session-state='idle']")
      .waitFor({ timeout: 10_000 });
    const terminalRefreshElapsedMs = Date.now() - terminalRefreshStartedAt;
    const terminalRuntimeRefreshedWithinOneSecond = terminalRefreshElapsedMs <= 1_000;
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
    assertIncludes(startingText, "GATE 2（実行） / 開始中", "optimistic launch row state");
    await waitFor(() => indexCalls > beforeLaunch, "launch acceptance refresh");

    const beforeTerminal = indexCalls;
    terminalReleased = true;
    releaseTerminal();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await waitFor(() => indexCalls > beforeTerminal, "terminal transition refresh");
    await page.waitForFunction(
      (id) => document.querySelector(`#trial-session-${id}`)?.innerText.includes("GATE 3（完了） / 完了"),
      createdSessionId,
    );
    const terminalText = await launchedRow.innerText();
    assertIncludes(terminalText, "GATE 3（完了） / 完了", "terminal history state");

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
    const terminalRowSelection = await launchedRow.evaluate(
      (row, id) => ({
        aria_current: row.getAttribute("aria-current"),
        highlighted:
          row.getAttribute("data-session-id") === id && row.classList.contains("highlight"),
      }),
      createdSessionId,
    );
    assert(
      terminalRowSelection.highlighted && terminalRowSelection.aria_current === "true",
      "terminal history link did not expose the selected active row",
    );
    const sessionTimes = await launchedRow.locator("time").allInnerTexts();
    const expectedSessionTimes = await page.evaluate((summary) => {
      const formatter = new Intl.DateTimeFormat("ja-JP", {
        dateStyle: "medium",
        timeStyle: "short",
      });
      return [
        `開始: ${formatter.format(new Date(summary.started_epoch_seconds * 1_000))}`,
        `最終更新: ${formatter.format(new Date(summary.modified_epoch_seconds * 1_000))}`,
      ];
    }, terminalSummary(createdSessionId));
    const timeLabelsUseSharedJaJpFormat =
      JSON.stringify(sessionTimes) === JSON.stringify(expectedSessionTimes);

    const freshnessBeforeFailure = await page
      .locator("[data-testid='trial-session-freshness']")
      .innerText();
    failNextIndex = true;
    await page.locator("[data-testid='refresh-trial-sessions']").click();
    const refreshError = page.locator(".session-index-error[role='alert']");
    await refreshError.waitFor();
    assertIncludes(await refreshError.innerText(), "最後に取得できた一覧", "stale data guidance");
    assert((await launchedRow.count()) === 1, "refresh failure removed the last successful row");
    const refreshErrorRetainedLastSuccess = (await launchedRow.count()) === 1;
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
    const focusRevalidated = indexCalls > beforeFocus;
    const visibilityRevalidated = indexCalls > beforeVisibility;

    const reconnectLink = launchedRow.locator("[data-testid='session-reconnect-link']");
    assert(
      (await reconnectLink.getAttribute("href")) === `?session=${createdSessionId}`,
      "session deep link changed",
    );
    const runtimeMaxConcurrentRequests = maxConcurrentRuntimeCalls;
    const requestOffset = sessionRequests.length;
    const beforeReconnect = indexCalls;
    await Promise.all([
      page.waitForNavigation({ waitUntil: "networkidle" }),
      reconnectLink.click(),
    ]);
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await waitFor(() => indexCalls > beforeReconnect, "automatic reconnect success refresh");
    const automaticReconnectRestoredResult =
      new URL(page.url()).searchParams.get("session") === createdSessionId;
    const storageKey = await page.evaluate(
      (token) => Object.entries(sessionStorage).find(([, value]) => value === token)?.[0] ?? null,
      trialToken,
    );
    assert(storageKey !== null, "Trial token storage key was not retained after reconnect");
    await page.evaluate(
      ({ key, token }) => sessionStorage.setItem(key, token),
      { key: storageKey, token: rejectedTrialToken },
    );
    await page.reload({ waitUntil: "networkidle" });
    await page.waitForFunction(
      () => document.querySelector("[data-testid='trial-token']")?.value === "",
    );
    const rejectedTokenRemoved = await page.evaluate(
      () => !Object.values(sessionStorage).some((value) => value.includes("-wrong")),
    );
    await page.locator("[data-testid='trial-token']").fill(trialToken);
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const reconnectRequests = sessionRequests.slice(requestOffset);
    const reconnectGetOnly =
      reconnectRequests.length > 0 && reconnectRequests.every((request) => request.method === "GET");

    const runtimeBadgeReconnect = await probeRuntimeBadgeReconnect(
      browser,
      origin,
      basePath,
    );

    return {
      initial_index_calls: initialIndexCalls,
      runtime_calls: runtimeCalls,
      runtime_max_concurrent_requests: runtimeMaxConcurrentRequests,
      runtime_paused_while_hidden: runtimePausedWhileHidden,
      runtime_resumed_when_visible: runtimeResumedWhenVisible,
      terminal_refresh_elapsed_ms: terminalRefreshElapsedMs,
      terminal_runtime_refreshed_within_one_second: terminalRuntimeRefreshedWithinOneSecond,
      runtime_lease_refresh_calls: runtimeLeaseRefreshCalls,
      final_index_calls: indexCalls,
      no_periodic_index_polling: noPeriodicIndexPolling,
      optimistic_starting_text: startingText,
      terminal_text: terminalText,
      runtime_live_region: runtimeLiveRegion,
      terminal_row_highlighted: terminalRowSelection.highlighted,
      terminal_row_aria_current: terminalRowSelection.aria_current,
      time_labels_use_shared_ja_jp_format: timeLabelsUseSharedJaJpFormat,
      refresh_error_retained_last_success: refreshErrorRetainedLastSuccess,
      focus_revalidated: focusRevalidated,
      visibility_revalidated: visibilityRevalidated,
      reconnect_requests: reconnectRequests,
      reconnect_get_only: reconnectGetOnly,
      automatic_reconnect_restored_result: automaticReconnectRestoredResult,
      rejected_token_removed: rejectedTokenRemoved,
      runtime_badge_navigated: runtimeBadgeReconnect.navigated,
      runtime_badge_reconnected: runtimeBadgeReconnect.reconnected,
      runtime_reconnect_requests: runtimeBadgeReconnect.requests,
      ok:
        noPeriodicIndexPolling &&
        reconnectGetOnly &&
        automaticReconnectRestoredResult &&
        rejectedTokenRemoved &&
        runtimeMaxConcurrentRequests === 1 &&
        runtimePausedWhileHidden &&
        runtimeResumedWhenVisible &&
        terminalRuntimeRefreshedWithinOneSecond &&
        runtimeLiveRegionIsPoliteAtomic &&
        terminalRowSelection.highlighted &&
        terminalRowSelection.aria_current === "true" &&
        timeLabelsUseSharedJaJpFormat &&
        refreshErrorRetainedLastSuccess &&
        focusRevalidated &&
        visibilityRevalidated &&
        runtimeBadgeReconnect.ok,
    };
  } finally {
    await page.close();
  }
}

async function probeRuntimeBadgeReconnect(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const requests = [];
  let indexCalls = 0;
  try {
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      const method = request.method();
      if (pathname.endsWith("/api/runtime-status") && method === "GET") {
        await json(route, 200, {
          trial_available: true,
          trial_token_auth_enabled: true,
          session: { id: existingSessionId, state: "running" },
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
      if (pathname.endsWith("/api/sessions") && method === "GET") {
        indexCalls += 1;
        await json(route, 200, {
          sessions: [terminalSummary(existingSessionId)],
          lease: { status: "idle" },
        });
        return;
      }
      if (pathname.endsWith(`/api/sessions/${existingSessionId}`) && method === "GET") {
        requests.push({ method, pathname });
        await json(route, 200, terminalSession(existingSessionId));
        return;
      }
      if (
        pathname.endsWith(`/api/sessions/${existingSessionId}/artifacts`) &&
        method === "GET"
      ) {
        await json(route, 200, []);
        return;
      }
      await json(route, 404, { error: `unexpected runtime badge API: ${method} ${pathname}` });
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-token']").fill(trialToken);
    await waitFor(() => indexCalls > 0, "runtime badge authentication");
    await page.goto(new URL(`${prefix}runs/`, origin).href, { waitUntil: "networkidle" });
    const runtimeStatus = page.locator("[data-testid='runtime-status']");
    await runtimeStatus.waitFor();
    const runtimeLink = page.locator("[data-testid='runtime-session-link']");
    assert(
      (await runtimeLink.count()) === 1,
      `running runtime badge was not linked: ${await runtimeStatus.innerText()}`,
    );
    const expectedHref = `${prefix}try/?session=${existingSessionId}`;
    const hrefMatches = (await runtimeLink.getAttribute("href")) === expectedHref;
    await Promise.all([
      page.waitForNavigation({ waitUntil: "domcontentloaded" }),
      runtimeLink.click(),
    ]);
    await waitFor(() => requests.length > 0, "runtime badge reconnect GET");
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const navigated =
      new URL(page.url()).pathname === new URL(expectedHref, origin).pathname &&
      new URL(page.url()).searchParams.get("session") === existingSessionId;
    const reconnected = requests.every((request) => request.method === "GET");
    return {
      href: await runtimeLink.getAttribute("href"),
      navigated,
      reconnected,
      requests,
      ok: hrefMatches && navigated && reconnected,
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
        "実行ルート / .commandagent/runs",
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
        (expectedRepositoryCount > 0 || repositoryEmpty.includes("リポジトリ実行記録なし")) &&
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

async function probeResourceRevalidation(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const retainedRun = repositoryRun("resource-retained", 1_723_769_600);
  const refreshedRun = repositoryRun("resource-refreshed", 1_723_769_660);
  let runs = [retainedRun];
  let runsCalls = 0;
  let failNextRuns = false;

  try {
    await page.route("**/api/runs", async (route) => {
      runsCalls += 1;
      if (failNextRuns) {
        failNextRuns = false;
        await json(route, 503, {
          code: "synthetic_resource_refresh_failed",
          error: "synthetic resource refresh failed",
        });
        return;
      }
      await json(route, 200, { runs, total: runs.length });
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(prefix, origin).href, { waitUntil: "networkidle" });
    const retainedRow = page.locator(`.runs-panel .run-row[data-run-id='${retainedRun.id}']`);
    await retainedRow.waitFor();
    const initialCalls = runsCalls;

    failNextRuns = true;
    await setDocumentVisibility(page, "hidden");
    await setDocumentVisibility(page, "visible");
    await waitFor(() => runsCalls > initialCalls, "visible resource revalidation");
    await page.locator(".runs-panel [role='alert']").waitFor();
    const failureRetainedPreviousData = (await retainedRow.count()) === 1;

    runs = [refreshedRun, retainedRun];
    const beforeFocus = runsCalls;
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    await waitFor(() => runsCalls > beforeFocus, "focused resource revalidation");
    await page.locator(`.runs-panel .run-row[data-run-id='${refreshedRun.id}']`).waitFor();
    const focusLoadedFreshData =
      (await page.locator(".runs-panel .run-row").count()) === 2 &&
      (await page.locator(".runs-panel [role='alert']").count()) === 0;

    return {
      initial_calls: initialCalls,
      final_calls: runsCalls,
      visibility_revalidated: runsCalls > initialCalls,
      failure_retained_previous_data: failureRetainedPreviousData,
      focus_loaded_fresh_data: focusLoadedFreshData,
      ok: failureRetainedPreviousData && focusLoadedFreshData,
    };
  } finally {
    await page.close();
  }
}

function repositoryRun(id, modifiedEpochSeconds) {
  return {
    id,
    modified_epoch_seconds: modifiedEpochSeconds,
    report_path: `workspace/management/runs/${id}/acceptance.md`,
    status: "completed",
    status_text: "検証済み",
    state: "pass",
  };
}

function isRuntimeStatusRequest(request) {
  return (
    request.method() === "GET" &&
    new URL(request.url()).pathname.endsWith("/api/runtime-status")
  );
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
      workspace: "<execution-root>",
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
    started_epoch_seconds: 1_723_769_600,
    average_duration_seconds: null,
    gate: "gate_3",
    status: "completed",
    verdict: "pass",
    assurance: "full",
    phases: [],
    event_count: 1,
    acceptance_sheet: "# Synthetic acceptance\n\nPASS",
    section5: "PASS",
    events_path: `.commandagent/runs/${id}/events.jsonl`,
    identity: syntheticProposal().identity,
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

async function setDocumentVisibility(page, visibilityState) {
  await page.evaluate((state) => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => state,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  }, visibilityState);
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
