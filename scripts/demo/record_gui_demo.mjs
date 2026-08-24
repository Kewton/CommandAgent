// Record a real GUI Trial walkthrough against a running gui_server.
// Produces: <out>/frames/*.png + frames.txt, <out>/gui-demo.gif, and named
// tutorial screenshots under <out>/shots/. Nothing is mocked: the delegated
// commandagent run is real and the GIF shows whatever the run produced.
import { createRequire } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { homedir } from "node:os";
import { spawnSync } from "node:child_process";

const require = createRequire(import.meta.url);
const playwrightPath =
  process.env.COMMANDAGENT_PLAYWRIGHT_PATH ??
  join(homedir(), ".anvil", "tools", "interaction-probe", "node_modules", "playwright");
const { chromium } = require(playwrightPath);

const args = Object.fromEntries(
  process.argv.slice(2).map((arg) => {
    const [key, ...rest] = arg.replace(/^--/, "").split("=");
    return [key, rest.join("=") || "true"];
  }),
);
const BASE = args.base ?? "http://127.0.0.1:4173";
const OUT = resolve(args.out ?? "gui-demo");
const MODEL = args.model ?? "qwen3.8:27b-mlx";
const TIMEOUT_MS = Number(args["timeout-ms"] ?? 1_800_000);
const FFMPEG = args.ffmpeg ?? "ffmpeg";
mkdirSync(join(OUT, "frames"), { recursive: true });
mkdirSync(join(OUT, "shots"), { recursive: true });

const frames = [];
let frameIndex = 0;
const log = (...items) => console.log(new Date().toISOString(), ...items);

async function frame(page, holdSeconds, shotName) {
  const path = join(OUT, "frames", `f${String(frameIndex++).padStart(4, "0")}.png`);
  await page.screenshot({ path, fullPage: false });
  frames.push({ path, duration: holdSeconds });
  if (shotName) await page.screenshot({ path: join(OUT, "shots", `${shotName}.png`), fullPage: true });
}

const browser = await chromium.launch();
const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1, locale: "ja-JP" });
const page = await context.newPage();

// 1. overview with the first-run card
await page.goto(`${BASE}/`, { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await frame(page, 2.5, "gui-01-overview");

// 2. sample goal -> trial form
await page.getByRole("link", { name: /サンプル目標/ }).click();
await page.waitForLoadState("networkidle");
await page.waitForTimeout(700);
await frame(page, 1.8, "gui-02-trial-form");
const modelInputs = page.locator('input[placeholder="正確なモデル ID"]');
for (let i = 0; i < (await modelInputs.count()); i += 1) {
  await modelInputs.nth(i).click();
  await modelInputs.nth(i).pressSequentially(MODEL, { delay: 35 });
}
await page.waitForTimeout(400);
await frame(page, 1.5, "gui-03-trial-form-filled");

// 3. Gate 1
await page.getByRole("button", { name: /契約と見積りを確認/ }).click();
await page.waitForSelector('[data-testid="gate-one-card"]');
await page.waitForTimeout(900);
await frame(page, 3.5, "gui-04-gate1");
await page.locator(".confirm-check input[type=checkbox]").check();
await page.waitForTimeout(500);
await frame(page, 1.2);
await page.getByRole("button", { name: /確認して CLI を実行/ }).click();
await page.waitForTimeout(2_000);
await frame(page, 2.0, "gui-05-gate2-start");
log("launched", page.url());

// 4. Gate 2 progress: capture whenever the phase list changes
let lastPhases = "";
const started = Date.now();
let terminal = false;
while (Date.now() - started < TIMEOUT_MS) {
  await page.waitForTimeout(10_000);
  const state = await page.evaluate(() => ({
    phases: [...document.querySelectorAll(".phase-row")].map((row) => row.innerText.replace(/\s+/g, " ")).join(" | "),
    terminal: document.querySelector('[data-testid="terminal-result-heading"]') !== null,
    elapsed: document.body.innerText.match(/経過時間\s*\n?\s*(\d\d:\d\d:\d\d)/)?.[1] ?? "",
  }));
  if (state.terminal) {
    terminal = true;
    break;
  }
  if (state.phases !== lastPhases) {
    lastPhases = state.phases;
    await frame(page, 1.4);
    log("phases:", state.phases, "elapsed", state.elapsed);
  }
}
log("terminal reached:", terminal);

// 5. result
await page.waitForTimeout(800);
await frame(page, 2.0, "gui-06-result-top");
const verdict = page.locator('[data-testid="terminal-result-heading"]');
await verdict.scrollIntoViewIfNeeded();
await page.evaluate(() => window.scrollBy(0, -120));
await page.waitForTimeout(600);
await frame(page, 4.0, "gui-07-result");
const summaryButton = page.locator(".session-file-list button", { hasText: "summary.md" });
if (await summaryButton.count()) {
  await summaryButton.first().click();
  await page.waitForTimeout(900);
  await frame(page, 3.0, "gui-08-summary");
}
await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
await page.waitForTimeout(500);
await frame(page, 2.5, "gui-09-history");

// 6. extensions catalog for the tutorial
await page.goto(`${BASE}/assets/`, { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await page.screenshot({ path: join(OUT, "shots", "gui-10-extensions.png"), fullPage: true });
await page.goto(`${BASE}/runs/`, { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await page.screenshot({ path: join(OUT, "shots", "gui-11-runs.png"), fullPage: true });
await browser.close();

// 7. assemble the GIF
const manifest = frames.map((f) => `file '${f.path}'\nduration ${f.duration.toFixed(3)}`);
manifest.push(`file '${frames[frames.length - 1].path}'`);
const listPath = join(OUT, "frames.txt");
writeFileSync(listPath, `${manifest.join("\n")}\n`);
const gif = join(OUT, "gui-demo.gif");
const filters =
  "fps=8,scale=1000:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle";
const result = spawnSync(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", listPath, "-vf", filters, "-loop", "0", gif], { stdio: "inherit" });
if (result.status !== 0) throw new Error(`ffmpeg failed with ${result.status}`);
log("wrote", gif, "frames", frames.length, "total", frames.reduce((s, f) => s + f.duration, 0).toFixed(1), "s", "terminal", terminal);
