import { spawn } from "node:child_process";
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
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
const validTrialToken = "commandagent-gui-storage-smoke-token-000000000081";
const rejectedTrialToken = `${validTrialToken}-rejected`;
const editedTrialToken = "edited-tab-token";
const trialTokenValues = [validTrialToken, rejectedTrialToken, editedTrialToken];
const cases = [
  { id: "root", buildBasePath: "/", serverBasePath: "/" },
  {
    id: "proxy-commandagent",
    buildBasePath: "/proxy/commandagent/",
    serverBasePath: "/proxy/commandagent",
  },
];

const scratchRoot = await mkdtemp(join(tmpdir(), "commandagent-gui-storage-smoke-"));
const fakeCli = join(scratchRoot, "fake-commandagent");
const require = createRequire(import.meta.url);
const { chromium } = require(managedPlaywrightPath);
const results = [];
let browser;

try {
  await writeFile(fakeCli, "#!/bin/sh\nexit 0\n");
  await chmod(fakeCli, 0o755);
  browser = await chromium.launch({ headless: true });

  for (const smokeCase of cases) {
    results.push(await runCase(smokeCase));
  }
} finally {
  await browser?.close();
  await rm(scratchRoot, { recursive: true, force: true });
}

const storageKeysAreIsolated =
  new Set(results.map((result) => result.storage_key)).size === cases.length;
const report = {
  schema_version: "commandagent.gui-trial-storage-smoke/v1",
  cases: results,
  storage_keys_are_isolated: storageKeysAreIsolated,
  ok: storageKeysAreIsolated && results.every((result) => result.ok),
};
if (outputDirectory !== null) {
  await mkdir(outputDirectory, { recursive: true });
  await writeFile(
    join(outputDirectory, "trial-storage-smoke.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}
console.log(JSON.stringify(report, null, 2));
if (!report.ok) process.exitCode = 1;

async function runCase(smokeCase) {
  await runChecked("npm", ["run", "build"], guiRoot, {
    ...process.env,
    GUI_BASE_PATH: smokeCase.buildBasePath,
  });
  const staticExportExcludesTokens = await treeExcludesTokens(join(guiRoot, "out"));
  const executionRoot = join(scratchRoot, smokeCase.id);
  await mkdir(executionRoot, { recursive: true });
  const server = await startServer(smokeCase.serverBasePath, executionRoot);
  const context = await browser.newContext();
  const consoleOutput = [];
  const page = await context.newPage();
  page.on("console", (entry) => consoleOutput.push(entry.text()));

  try {
    const prefix = displayBasePath(smokeCase.serverBasePath);
    const trialUrl = new URL(`${prefix}try/`, server.origin).href;
    const sessionsPath = `${prefix}api/sessions`;
    const storageKey = trialTokenStorageKey(smokeCase.serverBasePath);
    await page.goto(trialUrl, { waitUntil: "networkidle" });
    await page
      .locator("[data-testid='trial-profile'] option[value='python-cli']")
      .waitFor({ state: "attached" });
    const tokenInput = page.locator("[data-testid='trial-token']");
    const initialValueEmpty = (await tokenInput.inputValue()) === "";
    const renderedOutput = await page.content();

    await tokenInput.fill(editedTrialToken);
    const editedValuePersisted = await storageValue(page, storageKey, editedTrialToken);
    await tokenInput.fill("");
    const clearedValueRemoved = await storageValue(page, storageKey, null);

    const initialAccess = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === sessionsPath && response.status() === 200,
    );
    await tokenInput.fill(validTrialToken);
    await initialAccess;
    const authenticatedTrialAccess = true;
    const expectedStorageKeyUsed = await storageValue(page, storageKey, validTrialToken);

    const reloadAccess = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === sessionsPath && response.status() === 200,
    );
    await Promise.all([page.reload({ waitUntil: "networkidle" }), reloadAccess]);
    const reloadRestoredToken = (await tokenInput.inputValue()) === validTrialToken;

    const independentPage = await context.newPage();
    independentPage.on("console", (entry) => consoleOutput.push(entry.text()));
    await independentPage.goto(trialUrl, { waitUntil: "networkidle" });
    await independentPage.locator("[data-testid='trial-token']").waitFor();
    const independentTabEmpty =
      (await independentPage.locator("[data-testid='trial-token']").inputValue()) === "" &&
      (await storageValue(independentPage, storageKey, null));
    const independentUrl = independentPage.url();
    const independentLocalStorage = await independentPage.evaluate(() =>
      Object.values(localStorage),
    );
    await independentPage.close();

    const rejectionResponse = page.waitForResponse((response) => {
      const url = new URL(response.url());
      return url.pathname === sessionsPath && response.status() === 401;
    });
    await tokenInput.fill(rejectedTrialToken);
    await rejectionResponse;
    await page.waitForFunction(
      () => document.querySelector("[data-testid='trial-token']")?.value === "",
    );
    const errorText = (await page.locator(".trial-error[role='alert']").allInnerTexts()).join("\n");
    const rejectedValueRemoved = await storageValue(page, storageKey, null);

    const mainLocalStorage = await page.evaluate(() => Object.values(localStorage));
    const localStorageExcludesTokens = [...mainLocalStorage, ...independentLocalStorage].every(
      excludesTokens,
    );
    const urlsExcludeTokens = [page.url(), independentUrl].every(excludesTokens);
    const renderedOutputExcludesTokens = excludesTokens(renderedOutput);
    const consoleAndErrorsExcludeTokens = [...consoleOutput, errorText].every(excludesTokens);
    const serverDiagnosticsExcludeTokens = excludesTokens(server.diagnostics());
    const ok =
      initialValueEmpty &&
      editedValuePersisted &&
      clearedValueRemoved &&
      authenticatedTrialAccess &&
      expectedStorageKeyUsed &&
      reloadRestoredToken &&
      independentTabEmpty &&
      rejectedValueRemoved &&
      localStorageExcludesTokens &&
      urlsExcludeTokens &&
      staticExportExcludesTokens &&
      renderedOutputExcludesTokens &&
      consoleAndErrorsExcludeTokens &&
      serverDiagnosticsExcludeTokens;

    return {
      id: smokeCase.id,
      base_path: smokeCase.buildBasePath,
      storage_key: storageKey,
      initial_value_empty: initialValueEmpty,
      edited_value_persisted: editedValuePersisted,
      cleared_value_removed: clearedValueRemoved,
      authenticated_trial_access: authenticatedTrialAccess,
      expected_storage_key_used: expectedStorageKeyUsed,
      reload_restored_token: reloadRestoredToken,
      independent_tab_empty: independentTabEmpty,
      rejected_value_removed: rejectedValueRemoved,
      local_storage_excludes_tokens: localStorageExcludesTokens,
      urls_exclude_tokens: urlsExcludeTokens,
      static_export_excludes_tokens: staticExportExcludesTokens,
      rendered_output_excludes_tokens: renderedOutputExcludesTokens,
      console_and_errors_exclude_tokens: consoleAndErrorsExcludeTokens,
      server_diagnostics_exclude_tokens: serverDiagnosticsExcludeTokens,
      ok,
    };
  } finally {
    await context.close();
    server.stop();
  }
}

async function storageValue(page, key, expected) {
  return page.evaluate(
    ({ storageKey, expectedValue }) =>
      sessionStorage.getItem(storageKey) === expectedValue,
    { storageKey: key, expectedValue: expected },
  );
}

function excludesTokens(value) {
  return trialTokenValues.every((token) => !value.includes(token));
}

async function treeExcludesTokens(root) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      if (!(await treeExcludesTokens(path))) return false;
    } else if (entry.isFile()) {
      const bytes = await readFile(path);
      if (trialTokenValues.some((token) => bytes.includes(Buffer.from(token)))) {
        return false;
      }
    }
  }
  return true;
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
      fakeCli,
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, GUI_TRIAL_TOKEN: validTrialToken },
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
      reject(new Error("GUI server start timed out"));
    }, 60_000);
    child.stdout.on("data", (chunk) => {
      diagnostics = `${diagnostics}${chunk}`.slice(-12_000);
      const match = chunk.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (match !== null) {
        clearTimeout(timer);
        resolveOrigin(match[1]);
      }
    });
    child.once("exit", (code) => {
      clearTimeout(timer);
      reject(new Error(`GUI server exited before startup with code ${code}`));
    });
  });
  return {
    diagnostics: () => diagnostics,
    origin,
    stop: () => child.kill("SIGTERM"),
  };
}

async function runChecked(command, arguments_, cwd, env) {
  await new Promise((resolveRun, reject) => {
    const child = spawn(command, arguments_, { cwd, env, stdio: "inherit" });
    child.once("exit", (code) => {
      if (code === 0) resolveRun();
      else reject(new Error(`${command} exited with code ${code}`));
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
  return index === -1 ? null : arguments_[index + 1] ?? null;
}
