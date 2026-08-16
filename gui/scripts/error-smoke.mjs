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
  const spec = await page.evaluate(() => {
    const inputs = Array.from(document.querySelectorAll(".trial-fields input"));
    const selects = Array.from(document.querySelectorAll(".trial-fields select"));
    return {
      goal: document.querySelector("[data-testid='trial-goal']")?.value,
      profile: selects[0]?.value,
      provider: selects[1]?.value,
      model: inputs[0]?.value,
      planner_provider: selects[1]?.value,
      planner_model: inputs[1]?.value,
    };
  });
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
      return { body: await response.json(), status: response.status };
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
  await error.waitFor();
  const conflictGuidance = await error.innerText();
  requireIncludes(conflictGuidance, created.body.id, "running-session ID guidance");
  requireIncludes(conflictGuidance, "再接続", "running-session reconnect guidance");
  const reconnect = page.locator("[data-testid='reconnect-session-link']");
  await reconnect.waitFor();
  requireIncludes(await reconnect.innerText(), created.body.id, "reconnect link session ID");
  await reconnect.click();
  await page.locator("[data-testid='session-progress']").waitFor();
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 3_500));

  console.log(
    JSON.stringify(
      {
        origin_guidance: originGuidance,
        running_session: created.body.id,
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

function requireIncludes(value, expected, label) {
  if (!value.includes(expected)) {
    throw new Error(`${label} did not include ${JSON.stringify(expected)}: ${value}`);
  }
}
