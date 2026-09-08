/**
 * Run only from the supported Browser Node tool, after reading that browser's
 * complete documentation. Pass the existing binding: never bootstrap/reset here.
 * The caller must inspect the page/action before preparing the request.
 */
import { readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { createHash } from "node:crypto";

export async function probeBrowser(browser, ticketPath, observedConditions = {}) {
  const ticketBytes = await readFile(ticketPath);
  const ticket = JSON.parse(ticketBytes);
  const root = dirname(ticketPath);
  const request = ticket.request;
  if (request.method !== "browser-plugin" || ticket.entry_point.status !== "present") {
    throw new Error("A grounded current-session plugin entry point is required");
  }
  const state = JSON.parse(await readFile(join(root, "../state.json"), "utf8"));
  const reservation = state.attempts.find(a => a.id === ticket.id);
  if (reservation?.status !== "pending" || reservation.ticket_sha256 !==
      createHash("sha256").update(ticketBytes).digest("hex")) {
    throw new Error("A current, intact attempt reservation is required");
  }
  await writeFile(join(root, "probe-started.json"), JSON.stringify({ ticket_id: ticket.id }), { flag: "wx" });
  const result = { ticket_id: ticket.id, conditions: observedConditions, stage: "tabs" };
  let tab;
  try {
    result.tabs_count = (await browser.tabs.list()).length;
    // Zero tabs does not invalidate browser. Create only our own disposable tab.
    tab = await browser.tabs.new();
    result.stage = "navigate";
    await tab.goto(request.target_url);
    result.url = await tab.url();
    result.stage = "read";
    const before = await tab.playwright.domSnapshot();
    await writeFile(join(root, "before.txt"), before, { flag: "wx" });
    result.before = "before.txt";
    const action = request.action;
    if (!before.includes(action.before) || before.includes(action.after)) {
      throw new Error("Declared pre-interaction state was not observed");
    }
    const target = tab.playwright.getByRole(action.role, { name: action.name, exact: true });
    if (!(await target.isVisible())) throw new Error("Declared harmless action is not visible");
    result.stage = "interaction";
    await target.click();
    await tab.playwright.getByText(action.after, { exact: true }).waitFor({ state: "visible", timeoutMs: 10000 });
    const after = await tab.playwright.domSnapshot();
    await writeFile(join(root, "after.txt"), after, { flag: "wx" });
    result.after = "after.txt";
    result.action = action;
    result.url = await tab.url();
    result.stage = "screenshot";
    const screenshot = await tab.screenshot({ fullPage: false });
    const imageName = screenshot[0] === 0xff && screenshot[1] === 0xd8
      ? "screenshot.jpg" : "screenshot.png";
    await writeFile(join(root, imageName), screenshot, { flag: "wx" });
    result.screenshot = imageName;
    result.stage = "complete";
  } catch (error) {
    result.error = String(error);
  } finally {
    if (tab) {
      try {
        await tab.close();
        result.owned_tab_closed = true;
      } catch (error) {
        result.cleanup_error = String(error);
      }
    }
  }
  await writeFile(join(root, "observation.json"), JSON.stringify(result, null, 2) + "\n", { flag: "wx" });
  return result;
}
