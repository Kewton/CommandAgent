import { readFile } from "node:fs/promises";
import { join } from "node:path";

const RUN_WORKSPACE_DIRECTORIES = [".commandagent", ".anvil"];

export function runEvidenceCandidates(executionRoot, sessionId, filename) {
  return RUN_WORKSPACE_DIRECTORIES.map((workspaceDirectory) =>
    join(executionRoot, workspaceDirectory, "runs", sessionId, filename),
  );
}

export async function readRunEvidence(executionRoot, sessionId, filename) {
  const candidates = runEvidenceCandidates(executionRoot, sessionId, filename);
  let canonicalMissing;

  for (const path of candidates) {
    try {
      return { bytes: await readFile(path), path };
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      canonicalMissing ??= error;
    }
  }

  const error = new Error(`run evidence not found; checked: ${candidates.join(", ")}`);
  error.code = "ENOENT";
  error.cause = canonicalMissing;
  throw error;
}
