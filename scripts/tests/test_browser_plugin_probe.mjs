import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { probeBrowser } from "../browser_preflight/plugin_probe.mjs";

async function fixture(t, options = {}) {
  const root = await mkdtemp(join(tmpdir(), "browser-preflight-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const attempt = join(root, "attempt");
  await mkdir(attempt);
  const request = { method: "browser-plugin", target_url: "http://127.0.0.1:1234/",
    action: { role: "button", name: "Check browser", before: "Ready", after: "Confirmed" } };
  const ticket = { id: "attempt", request, entry_point: { status: "present" } };
  const bytes = JSON.stringify(ticket);
  const path = join(attempt, "ticket.json");
  await writeFile(path, bytes);
  await writeFile(join(root, "state.json"), JSON.stringify({ attempts: [{ id: ticket.id,
    status: "pending", ticket_sha256: createHash("sha256").update(bytes).digest("hex") }] }));
  const calls = [];
  let clicked = false;
  const tab = {
    goto: async url => { calls.push(["goto", url]); },
    url: async () => request.target_url,
    close: async () => { calls.push(["close"]); },
    screenshot: async () => {
      calls.push(["screenshot"]);
      if (options.imageFails) throw new Error("Image transport failed");
      return new Uint8Array([1, 2, 3]);
    },
    playwright: {
      domSnapshot: async () => clicked ? "Confirmed" : "Ready",
      getByRole: (role, selector) => {
        assert.equal(role, "button");
        assert.equal(selector.name, "Check browser");
        return { isVisible: async () => true, click: async () => { clicked = true; calls.push(["click"]); } };
      },
      getByText: () => ({ waitFor: async () => {} }),
    },
  };
  const browser = { tabs: {
    list: async () => { calls.push(["list"]); return []; },
    new: async () => { calls.push(["new"]); return tab; },
  } };
  return { path, calls, browser, attempt };
}

test("empty tabs reuse the supplied browser and close only the new tab", async t => {
  const f = await fixture(t);
  const result = await probeBrowser(f.browser, f.path);
  assert.equal(result.stage, "complete");
  assert.equal(result.tabs_count, 0);
  assert.equal(result.owned_tab_closed, true);
  assert.deepEqual(f.calls.map(c => c[0]), ["list", "new", "goto", "click", "screenshot", "close"]);
  assert.equal(JSON.parse(await readFile(join(f.attempt, "observation.json"))).url, "http://127.0.0.1:1234/");
});

test("image failures are saved without retry or reconnect", async t => {
  const f = await fixture(t, { imageFails: true });
  const result = await probeBrowser(f.browser, f.path);
  assert.equal(result.stage, "screenshot");
  assert.equal(result.error, "Error: Image transport failed");
  assert.equal(result.owned_tab_closed, true);
  assert.equal(f.calls.filter(c => c[0] === "screenshot").length, 1);
});

test("one reservation cannot execute browser operations twice", async t => {
  const f = await fixture(t);
  await probeBrowser(f.browser, f.path);
  const count = f.calls.length;
  await assert.rejects(probeBrowser(f.browser, f.path), /EEXIST/);
  assert.equal(f.calls.length, count);
});

test("unproven entry point fails before using the browser", async t => {
  const f = await fixture(t);
  const ticket = JSON.parse(await readFile(f.path));
  ticket.entry_point.status = "unknown";
  await writeFile(f.path, JSON.stringify(ticket));
  await assert.rejects(probeBrowser(f.browser, f.path), /grounded current-session/);
  assert.deepEqual(f.calls, []);
});
