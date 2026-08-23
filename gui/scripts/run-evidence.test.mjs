import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { readRunEvidence, runEvidenceCandidates } from "./run-evidence.mjs";

const sessionId = "issue-337";

test("reads canonical .commandagent run evidence", async (context) => {
  const root = await scratchRoot(context);
  const [canonical] = runEvidenceCandidates(root, sessionId, "events.jsonl");
  await mkdir(join(root, ".commandagent", "runs", sessionId), { recursive: true });
  await writeFile(canonical, "canonical\n");

  const evidence = await readRunEvidence(root, sessionId, "events.jsonl");

  assert.equal(evidence.path, canonical);
  assert.equal(evidence.bytes.toString("utf8"), "canonical\n");
});

test("falls back to legacy .anvil run evidence", async (context) => {
  const root = await scratchRoot(context);
  const [, legacy] = runEvidenceCandidates(root, sessionId, "events.jsonl");
  await mkdir(join(root, ".anvil", "runs", sessionId), { recursive: true });
  await writeFile(legacy, "legacy\n");

  const evidence = await readRunEvidence(root, sessionId, "events.jsonl");

  assert.equal(evidence.path, legacy);
  assert.equal(evidence.bytes.toString("utf8"), "legacy\n");
});

test("prefers canonical evidence when both paths exist", async (context) => {
  const root = await scratchRoot(context);
  const [canonical, legacy] = runEvidenceCandidates(root, sessionId, "events.jsonl");
  await mkdir(join(root, ".commandagent", "runs", sessionId), { recursive: true });
  await mkdir(join(root, ".anvil", "runs", sessionId), { recursive: true });
  await writeFile(canonical, "canonical\n");
  await writeFile(legacy, "legacy\n");

  const evidence = await readRunEvidence(root, sessionId, "events.jsonl");

  assert.equal(evidence.path, canonical);
  assert.equal(evidence.bytes.toString("utf8"), "canonical\n");
});

test("reports both checked paths when evidence is missing", async (context) => {
  const root = await scratchRoot(context);
  const candidates = runEvidenceCandidates(root, sessionId, "events.jsonl");

  await assert.rejects(
    readRunEvidence(root, sessionId, "events.jsonl"),
    (error) =>
      error.code === "ENOENT" && candidates.every((candidate) => error.message.includes(candidate)),
  );
});

async function scratchRoot(context) {
  const root = await mkdtemp(join(tmpdir(), "commandagent-run-evidence-test-"));
  context.after(() => rm(root, { recursive: true, force: true }));
  return root;
}
