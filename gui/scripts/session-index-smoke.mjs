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
  const liveTaskPayloadBytes = Buffer.byteLength(JSON.stringify(runningSession(createdSessionId)));
  const terminalTaskPayloadBytes = Buffer.byteLength(
    JSON.stringify(terminalSession(createdSessionId)),
  );
  const taskPayloadsBounded =
    liveTaskPayloadBytes < 128 * 1024 && terminalTaskPayloadBytes < 128 * 1024;
  assert(taskPayloadsBounded, "100-task polling projection exceeded 128 KiB");
  let indexSessions = [];
  let indexCalls = 0;
  let runtimeCalls = 0;
  let maxConcurrentRuntimeCalls = 0;
  const activeRuntimeRequests = new Set();
  let runtimeSession = null;
  let failNextIndex = false;
  let terminalReleased = false;
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
        indexSessions = [runningSummary(createdSessionId)];
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
        if (terminalReleased) {
          indexSessions = [terminalSummary(createdSessionId)];
          await json(route, 200, terminalSession(createdSessionId));
        } else {
          await json(route, 200, runningSession(createdSessionId));
        }
        return;
      }
      if (pathname.endsWith(`/api/sessions/${createdSessionId}/artifacts`)) {
        await json(route, 200, []);
        return;
      }
      if (pathname.endsWith(`/api/sessions/${createdSessionId}/events`) && method === "GET") {
        await json(route, 200, {
          path: "events.jsonl",
          content: '{"event":"plan_step_failed","step_execution_id":"terminal-step-3"}\n',
        });
        return;
      }
      await json(route, 404, { error: `unexpected mocked API: ${method} ${pathname}` });
    });

    const prefix = displayBasePath(basePath);
    await page.goto(new URL(`${prefix}try/history/`, origin).href, { waitUntil: "networkidle" });
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
    const runtimeMaxConcurrentRequests = maxConcurrentRuntimeCalls;

    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    assert(
      (await page.locator("[data-testid='trial-session-index']").count()) === 0,
      "compose route retained the Trial history list",
    );
    await page.locator("[data-testid='trial-goal']").fill("Synthetic live Trial history probe");
    await page.locator("[data-testid='trial-executor-model']").fill("synthetic-model");
    await page.locator("[data-testid='trial-planner-model']").fill("synthetic-model");
    await page.locator("[data-testid='check-contract']").click();
    await page.locator("[data-testid='gate-one-card']").waitFor();
    await page.locator("[data-testid='gate-one-confirm']").check();
    await page.locator("[data-testid='launch-session']").click();
    await page.locator("[data-testid='session-progress']").waitFor();
    const liveTaskProgress = page.locator("[data-testid='current-task-progress']");
    await liveTaskProgress.waitFor();
    const liveTaskText = await liveTaskProgress.innerText();
    assertIncludes(liveTaskText, "現在のフェーズ: implementation", "current task phase");
    assertIncludes(liveTaskText, "現在のタスク: task-100", "current task ID");
    assertIncludes(liveTaskText, "タスク 100 / 100", "current task position");
    const liveTaskCount = await page.locator("[data-testid='task-progress'] .task-list > li").count();
    assert(liveTaskCount === 100, `live task projection rendered ${liveTaskCount} tasks`);
    const launchUrl = new URL(page.url());
    const launchNavigatedToStatus =
      launchUrl.pathname === new URL(`${prefix}try/status/`, origin).pathname &&
      launchUrl.searchParams.get("session") === createdSessionId;

    await page.goto(new URL(`${prefix}try/history/`, origin).href, { waitUntil: "networkidle" });
    const launchedRow = page.locator(`#trial-session-${createdSessionId}`);
    await launchedRow.waitFor();
    const startingText = await launchedRow.innerText();
    assertIncludes(startingText, createdSessionId, "optimistic launch row ID");
    assertIncludes(startingText, "GATE 2（実行） / 実行中", "running history row state");
    const beforeTerminal = indexCalls;
    const activeRouteLink = launchedRow.locator("[data-testid='session-route-link']");
    const expectedStatusHref = `${prefix}try/status/?session=${createdSessionId}`;
    assert(
      (await activeRouteLink.getAttribute("href")) === expectedStatusHref,
      "running history row did not target status",
    );
    terminalReleased = true;
    await activeRouteLink.click();
    await page.locator("[data-testid='terminal-gate']").waitFor();
    await page.waitForFunction(
      (pathname) => window.location.pathname === pathname,
      new URL(`${prefix}try/history/detail/`, origin).pathname,
    );
    const detailUrl = new URL(page.url());
    const statusNavigatedToDetail =
      detailUrl.pathname === new URL(`${prefix}try/history/detail/`, origin).pathname &&
      detailUrl.searchParams.get("session") === createdSessionId;

    const terminalTaskProgress = page.locator("[data-testid='task-progress']");
    await terminalTaskProgress.waitFor();
    const terminalTaskCount = await terminalTaskProgress.locator(".task-list > li").count();
    const executionIntervalCount = await terminalTaskProgress
      .locator("[data-testid='task-execution']")
      .count();
    assert(terminalTaskCount === 101, `terminal task projection rendered ${terminalTaskCount}`);
    assert(executionIntervalCount === 2, "initial and continuation task intervals were merged");
    const terminalTaskText = await terminalTaskProgress.innerText();
    for (const label of [
      "completed（完了）",
      "short-circuited（実行省略）",
      "FAILED（失敗）",
      "interrupted（中断）",
    ]) {
      assertIncludes(terminalTaskText, label, `terminal task label ${label}`);
    }
    const failedTask = terminalTaskProgress.locator("[data-testid='task-failed']").first();
    const failedDetails = failedTask.locator("details");
    const failedSummary = failedTask.locator("summary");
    const failedTaskAutoExpanded =
      (await failedDetails.getAttribute("open")) !== null &&
      (await failedSummary.getAttribute("aria-expanded")) === "true";
    assert(failedTaskAutoExpanded, "FAILED task was not expanded with aria-expanded=true");
    assertIncludes(
      await failedTask.locator("[data-testid='task-failure-reason']").innerText(),
      "synthetic verification failed",
      "FAILED task reason",
    );
    const completedSummary = terminalTaskProgress
      .locator("[data-testid='task-completed'] summary")
      .first();
    await completedSummary.focus();
    await completedSummary.press("Enter");
    await page.waitForFunction(
      (element) => element?.getAttribute("aria-expanded") === "true",
      await completedSummary.elementHandle(),
    );
    const keyboardDisclosureExpanded =
      (await completedSummary.getAttribute("aria-expanded")) === "true";
    assert(keyboardDisclosureExpanded, "task disclosure was not keyboard operable");
    const headingHierarchy = await terminalTaskProgress.evaluate((node) =>
      Array.from(node.querySelectorAll("h3, h4")).map((heading) => heading.tagName.toLowerCase()),
    );
    const headingHierarchyValid =
      headingHierarchy[0] === "h3" && headingHierarchy.slice(1).every((tag) => tag === "h4");
    assert(headingHierarchyValid, `task heading hierarchy is invalid: ${headingHierarchy}`);
    const duplicateStepIdsKeptSeparate =
      (await terminalTaskProgress.locator("summary strong", { hasText: "same-step" }).count()) === 2;
    assert(duplicateStepIdsKeptSeparate, "duplicate Step IDs were merged across execution intervals");
    await failedTask.locator("[data-testid='task-evidence-link']").click();
    const taskEvidenceViewer = page.locator("[data-testid='trial-file-viewer']");
    await taskEvidenceViewer.waitFor();
    await page.waitForFunction(() =>
      document.querySelector("[data-testid='trial-file-viewer']")?.textContent?.includes(
        "plan_step_failed",
      ));
    assertIncludes(await taskEvidenceViewer.innerText(), "plan_step_failed", "task evidence link");
    await page.setViewportSize({ width: 390, height: 844 });
    const taskDetailMobileFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );
    await page.setViewportSize({ width: 1280, height: 1000 });

    const historyLink = page.locator("[data-testid='terminal-session-history-link']");
    const expectedHistoryHref = `${prefix}try/history/#trial-session-${createdSessionId}`;
    assert(
      (await historyLink.getAttribute("href")) === expectedHistoryHref,
      "terminal history link did not target the compact history row",
    );
    await historyLink.click();
    await waitFor(() => indexCalls > beforeTerminal, "terminal history refresh");
    await page.waitForFunction(
      (id) => document.querySelector(`#trial-session-${id}`)?.innerText.includes("GATE 3（完了） / 完了"),
      createdSessionId,
    );
    const terminalText = await launchedRow.innerText();
    assertIncludes(terminalText, "GATE 3（完了） / 完了", "terminal history state");

    assert(
      new URL(page.url()).hash === `#trial-session-${createdSessionId}`,
      "terminal history link did not navigate to its row",
    );
    const terminalRowSelection = await launchedRow.evaluate(
      (row, id) => ({
        targeted: row.getAttribute("data-session-id") === id && `#${row.id}` === location.hash,
        terminal: row.getAttribute("data-terminal"),
      }),
      createdSessionId,
    );
    assert(
      terminalRowSelection.targeted && terminalRowSelection.terminal === "true",
      "terminal history link did not expose the selected terminal row",
    );
    const terminalDiagnosticsCount = await launchedRow
      .locator("[data-testid='session-failure-diagnostics']")
      .count();
    const compactSummaryText = await launchedRow.innerText();
    assertIncludes(compactSummaryText, "プロファイル: python-cli", "history profile summary");
    assertIncludes(compactSummaryText, "目的: 作成", "history intent summary");
    assert(terminalDiagnosticsCount === 0, "history row expanded failure diagnostics");
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

    const reconnectLink = launchedRow.locator("[data-testid='session-route-link']");
    assert(
      (await reconnectLink.getAttribute("href")) ===
        `${prefix}try/history/detail/?session=${createdSessionId}`,
      "terminal session row did not target detail",
    );
    const requestOffset = sessionRequests.length;
    await Promise.all([
      page.waitForNavigation({ waitUntil: "networkidle" }),
      reconnectLink.click(),
    ]);
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const automaticReconnectRestoredResult =
      new URL(page.url()).pathname === new URL(`${prefix}try/history/detail/`, origin).pathname &&
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
    const routeOwnership = await probeRouteOwnership(browser, origin, basePath);

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
      terminal_row_targeted: terminalRowSelection.targeted,
      terminal_row_compact: terminalDiagnosticsCount === 0,
      launch_navigated_to_status: launchNavigatedToStatus,
      status_navigated_to_detail: statusNavigatedToDetail,
      live_task_text: liveTaskText,
      live_task_count: liveTaskCount,
      terminal_task_count: terminalTaskCount,
      live_task_payload_bytes: liveTaskPayloadBytes,
      terminal_task_payload_bytes: terminalTaskPayloadBytes,
      task_payloads_bounded: taskPayloadsBounded,
      execution_interval_count: executionIntervalCount,
      duplicate_step_ids_kept_separate: duplicateStepIdsKeptSeparate,
      failed_task_auto_expanded: failedTaskAutoExpanded,
      keyboard_disclosure_expanded: keyboardDisclosureExpanded,
      heading_hierarchy_valid: headingHierarchyValid,
      task_detail_mobile_fits: taskDetailMobileFits,
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
      route_ownership: routeOwnership,
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
        terminalRowSelection.targeted &&
        terminalDiagnosticsCount === 0 &&
        launchNavigatedToStatus &&
        statusNavigatedToDetail &&
        liveTaskCount === 100 &&
        terminalTaskCount === 101 &&
        taskPayloadsBounded &&
        executionIntervalCount === 2 &&
        duplicateStepIdsKeptSeparate &&
        failedTaskAutoExpanded &&
        keyboardDisclosureExpanded &&
        headingHierarchyValid &&
        taskDetailMobileFits &&
        timeLabelsUseSharedJaJpFormat &&
        refreshErrorRetainedLastSuccess &&
        focusRevalidated &&
        visibilityRevalidated &&
        runtimeBadgeReconnect.ok &&
        routeOwnership.ok,
    };
  } finally {
    await page.close();
  }
}

async function probeRouteOwnership(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const requests = [];
  let legacyTerminal = false;
  try {
    await page.route("**/api/**", async (route) => {
      const request = route.request();
      const pathname = new URL(request.url()).pathname;
      const method = request.method();
      if (pathname.endsWith("/api/runtime-status") && method === "GET") {
        await json(route, 200, {
          trial_available: true,
          trial_token_auth_enabled: true,
          session: null,
        });
        return;
      }
      if (pathname.endsWith("/api/trial-options") && method === "GET") {
        await json(route, 200, syntheticOptions());
        return;
      }
      if (pathname.endsWith("/api/pack-options") && method === "GET") {
        await json(route, 200, { packs: [] });
        return;
      }
      if (pathname.endsWith("/api/sessions") && method === "GET") {
        await json(route, 200, { sessions: [], lease: { status: "idle" } });
        return;
      }
      if (pathname.endsWith(`/api/sessions/${existingSessionId}`) && method === "GET") {
        requests.push({ method, pathname });
        await json(
          route,
          200,
          legacyTerminal
            ? legacyTerminalSession(existingSessionId)
            : runningSession(existingSessionId),
        );
        return;
      }
      if (
        pathname.endsWith(`/api/sessions/${existingSessionId}/artifacts`) &&
        method === "GET"
      ) {
        requests.push({ method, pathname });
        await json(route, 200, []);
        return;
      }
      await json(route, 404, { error: `unexpected route ownership API: ${method} ${pathname}` });
    });

    const prefix = displayBasePath(basePath);
    const routes = [
      { path: "try/", title: "トライアル実行指示 | CommandAgent", heading: "トライアル実行指示", active: "compose" },
      { path: "try/status/", title: "トライアル実行状況", heading: "トライアル実行状況", active: "status" },
      { path: "try/history/", title: "トライアル実行履歴", heading: "トライアル実行履歴", active: "history" },
      { path: "try/history/detail/", title: "トライアル実行結果詳細", heading: "トライアル実行結果詳細", active: "detail" },
    ];
    const states = [];
    for (const expected of routes) {
      const response = await page.goto(new URL(`${prefix}${expected.path}`, origin).href, {
        waitUntil: "networkidle",
      });
      const title = await page.title();
      const heading = await page.locator(".page-intro h1").innerText();
      const active = await page
        .locator("[data-testid='trial-page-nav'] [aria-current='page']")
        .getAttribute("data-testid");
      states.push({
        ...expected,
        http_status: response?.status() ?? null,
        observed_title: title,
        observed_heading: heading,
        observed_active: active,
        ok:
          response?.status() === 200 &&
          title === expected.title &&
          heading === expected.heading &&
          active === `trial-page-nav-${expected.active}`,
      });
    }

    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(new URL(`${prefix}try/history/`, origin).href, { waitUntil: "networkidle" });
    const mobileFits = await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    );

    await page.goto(new URL(`${prefix}try/`, origin).href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='trial-token']").fill(trialToken);
    const legacyRunning = new URL(`${prefix}try/`, origin);
    legacyRunning.searchParams.set("session", existingSessionId);
    await page.goto(legacyRunning.href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='session-progress']").waitFor();
    const legacyRunningUrl = new URL(page.url());
    const legacyRunningToStatus =
      legacyRunningUrl.pathname === new URL(`${prefix}try/status/`, origin).pathname &&
      legacyRunningUrl.searchParams.get("session") === existingSessionId;

    legacyTerminal = true;
    const legacyTerminalUrl = new URL(`${prefix}try/`, origin);
    legacyTerminalUrl.searchParams.set("session", existingSessionId);
    await page.goto(legacyTerminalUrl.href, { waitUntil: "networkidle" });
    await page.locator("[data-testid='terminal-gate']").waitFor();
    const legacyTaskUnsupported =
      (await page.locator("[data-testid='task-progress-unsupported']").count()) === 1 &&
      (await page.locator("[data-testid='task-progress-unsupported']").innerText()).includes(
        "不正確な成功件数は表示しません",
      );
    const redirectedTerminalUrl = new URL(page.url());
    const legacyTerminalToDetail =
      redirectedTerminalUrl.pathname ===
        new URL(`${prefix}try/history/detail/`, origin).pathname &&
      redirectedTerminalUrl.searchParams.get("session") === existingSessionId;
    const reconnectGetOnly = requests.length > 0 && requests.every(({ method }) => method === "GET");

    return {
      states,
      mobile_fits: mobileFits,
      legacy_running_to_status: legacyRunningToStatus,
      legacy_terminal_to_detail: legacyTerminalToDetail,
      legacy_task_unsupported: legacyTaskUnsupported,
      reconnect_get_only: reconnectGetOnly,
      requests,
      ok:
        states.every((state) => state.ok) &&
        mobileFits &&
        legacyRunningToStatus &&
        legacyTerminalToDetail &&
        legacyTaskUnsupported &&
        reconnectGetOnly,
    };
  } finally {
    await page.close();
  }
}

async function probeRuntimeBadgeReconnect(browser, origin, basePath) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const requests = [];
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
      if (pathname.endsWith(`/api/sessions/${existingSessionId}`) && method === "GET") {
        requests.push({ method, pathname });
        await json(route, 200, runningSession(existingSessionId));
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
    await page.goto(new URL(`${prefix}runs/`, origin).href, { waitUntil: "networkidle" });
    const runtimeStatus = page.locator("[data-testid='runtime-status']");
    await runtimeStatus.waitFor();
    const runtimeLink = page.locator("[data-testid='runtime-session-link']");
    assert(
      (await runtimeLink.count()) === 1,
      `running runtime badge was not linked: ${await runtimeStatus.innerText()}`,
    );
    const expectedHref = `${prefix}try/status/?session=${existingSessionId}`;
    const hrefMatches = (await runtimeLink.getAttribute("href")) === expectedHref;
    await Promise.all([
      page.waitForNavigation({ waitUntil: "domcontentloaded" }),
      runtimeLink.click(),
    ]);
    await waitFor(() => requests.length > 0, "runtime badge reconnect GET");
    await page.locator("[data-testid='session-progress']").waitFor();
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

      await page.goto(new URL(`${prefix}try/history/`, origin).href, { waitUntil: "networkidle" });
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
    profile: "python-cli",
    intent: "create",
    pack: {
      id: "cli-assist",
      version: "1.0.0",
      hash: "sha256:b1dcee70c1a0536954c25639e2d67508d8029328e414aaff030368e7fac844fd",
      source: "admitted",
      source_label: "承認済み",
    },
  };
}

function runningSummary(id) {
  return {
    id,
    started_epoch_seconds: 1_723_769_600,
    modified_epoch_seconds: 1_723_769_620,
    gate: "gate_2",
    status: "running",
    profile: "python-cli",
    intent: "create",
    pack: null,
  };
}

function runningSession(id) {
  return {
    ...terminalSession(id),
    gate: "gate_2",
    status: "running",
    verdict: null,
    assurance: null,
    acceptance_sheet: null,
    section5: null,
    task_progress: runningTaskProgress(),
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
    task_progress: terminalTaskProgress(),
    event_count: 205,
    acceptance_sheet: "# Synthetic acceptance\n\nPASS",
    section5: "PASS",
    events_path: `.commandagent/runs/${id}/events.jsonl`,
    identity: syntheticProposal().identity,
  };
}

function legacyTerminalSession(id) {
  return {
    ...terminalSession(id),
    task_progress: { status: "unsupported", executions: [] },
  };
}

function runningTaskProgress() {
  return {
    status: "supported",
    executions: [{
      execution_index: 1,
      plan_execution_id: "live-plan-execution",
      mode: "ultra-plan",
      phase_id: "implementation",
      total_steps: 100,
      tasks: Array.from({ length: 100 }, (_, index) =>
        taskStatus({
          executionId: `live-step-${index + 1}`,
          index: index + 1,
          status: index === 99 ? "running" : "completed",
          total: 100,
        })),
    }],
  };
}

function terminalTaskProgress() {
  const statuses = ["completed", "short_circuited", "failed", "interrupted"];
  return {
    status: "supported",
    executions: [
      {
        execution_index: 1,
        plan_execution_id: "initial-plan-execution",
        mode: "ultra-plan",
        phase_id: "implementation",
        total_steps: 100,
        tasks: Array.from({ length: 100 }, (_, index) =>
          taskStatus({
            executionId: `terminal-step-${index + 1}`,
            id: index === 0 ? "same-step" : `task-${index + 1}`,
            index: index + 1,
            status: statuses[index] ?? "completed",
            total: 100,
          })),
      },
      {
        execution_index: 2,
        plan_execution_id: "continuation-plan-execution",
        mode: "ultra-plan",
        phase_id: "follow-up",
        total_steps: 1,
        tasks: [taskStatus({
          executionId: "continuation-step-1",
          id: "same-step",
          index: 1,
          status: "short_circuited",
          total: 1,
        })],
      },
    ],
  };
}

function taskStatus({ executionId, id, index, status, total }) {
  const failed = status === "failed";
  const interrupted = status === "interrupted";
  const running = status === "running";
  const shortCircuited = status === "short_circuited";
  return {
    step_execution_id: executionId,
    step_index: index,
    total_steps: total,
    step_id: id ?? `task-${index}`,
    step_kind: index % 2 === 0 ? "verify" : "implement",
    status,
    outcome: running
      ? null
      : failed
        ? "verification_failed"
        : interrupted
          ? "interrupted"
          : shortCircuited
            ? "short_circuited"
            : "completed",
    verification_status: running
      ? null
      : failed
        ? "failed"
        : interrupted
          ? "not_run"
          : "passed",
    verification_failure_count: failed ? 1 : 0,
    verification_failures: failed ? ["synthetic check failed"] : [],
    verification_failures_truncated: false,
    changed_path_count: failed ? 1 : 0,
    changed_paths: failed ? ["src/synthetic.rs"] : [],
    changed_paths_truncated: false,
    repair_attempts: failed ? 2 : 0,
    failure_summary: failed ? "synthetic verification failed" : interrupted ? "interrupted by user" : null,
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
