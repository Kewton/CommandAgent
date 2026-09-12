// File-backed persistence layer with atomic writes.
//
// Data is stored as JSON files on disk:
//   - data/projects.json
//   - data/tasks.json
//
// Design goals:
//   1. Initialize an empty array `[]` only when the file is missing (ENOENT).
//   2. Distinguish "empty array" from "missing file": an existing file that
//      already contains `[]` is preserved, and a missing file is seeded with `[]`.
//   3. Parse / write errors surface as a thrown error so callers can return
//      a 5xx response WITHOUT overwriting the on-disk file.
//   4. Writes are atomic via a temp-file-then-rename pattern so a crash or
//      partial write cannot corrupt the canonical file.

import { randomUUID } from 'crypto';
import { promises as fs } from 'fs';
import * as path from 'path';
import type { Project, Task } from './types';

const DATA_DIR = path.join(process.cwd(), 'data');
const PROJECTS_FILE = path.join(DATA_DIR, 'projects.json');
const TASKS_FILE = path.join(DATA_DIR, 'tasks.json');

/**
 * Generate a reasonably unique id for new entities.
 * Uses the crypto module's UUID generator when available, falling back to a
 * timestamp + random suffix.
 */
export function generateId(): string {
  try {
    return randomUUID();
  } catch {
    return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
  }
}

/**
 * Ensure the data directory exists. Safe to call repeatedly.
 */
async function ensureDataDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

/**
 * Read and parse a JSON file of `T[]`.
 *
 * - If the file is missing (ENOENT), the directory is created, the file is
 *   seeded with an empty array `[]`, and `[]` is returned.
 * - If the file exists but cannot be parsed, the error is thrown and the
 *   file is NOT overwritten, so a transient parse error never destroys data.
 */
async function loadCollection<T>(filePath: string): Promise<T[]> {
  await ensureDataDir();
  try {
    const raw = await fs.readFile(filePath, 'utf8');
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed as T[];
    }
    // File exists but is not a JSON array. Do not overwrite; surface error.
    throw new Error(`Invalid JSON content (expected array) in ${path.basename(filePath)}`);
  } catch (err) {
    const e = err as NodeJS.ErrnoException;
    if (e.code === 'ENOENT') {
      // Missing file: seed with an empty array and persist it.
      const empty = '[]';
      await atomicWrite(filePath, empty);
      return [] as T[];
    }
    // Parse error or other read error: do not overwrite the file.
    throw err;
  }
}

/**
 * Persist a JSON array to disk atomically.
 *
 * Writes to a temporary sibling file first, then renames it over the target
 * path. `fs.rename` is atomic on POSIX filesystems, so the canonical file is
 * never observed in a half-written state. On any failure before the rename,
 * the original file remains intact.
 */
export async function atomicWrite(filePath: string, contents: string): Promise<void> {
  await ensureDataDir();
  const dir = path.dirname(filePath);
  const base = path.basename(filePath);
  // Temp file in the same directory so rename stays on the same filesystem.
  const tmpPath = path.join(dir, `${base}.${process.pid}.${Date.now()}.tmp`);
  try {
    await fs.writeFile(tmpPath, contents, 'utf8');
    await fs.rename(tmpPath, filePath);
  } catch (err) {
    // Best-effort cleanup of the temp file so it does not accumulate.
    try {
      await fs.rm(tmpPath, { force: true });
    } catch {
      /* ignore cleanup failure */
    }
    throw err;
  }
}

/**
 * Save a collection to disk. Throws on write failure WITHOUT partially
 * overwriting the canonical file (atomic via temp-file + rename).
 */
export async function saveCollection<T>(filePath: string, items: T[]): Promise<void> {
  const serialized = JSON.stringify(items, null, 2);
  await atomicWrite(filePath, serialized);
}

// --- Public API -----------------------------------------------------------

/** Load all projects from disk. */
export function loadProjects(): Promise<Project[]> {
  return loadCollection<Project>(PROJECTS_FILE);
}

/** Persist all projects to disk. */
export function saveProjects(projects: Project[]): Promise<void> {
  return saveCollection<Project>(PROJECTS_FILE, projects);
}

/** Load all tasks from disk. */
export function loadTasks(): Promise<Task[]> {
  return loadCollection<Task>(TASKS_FILE);
}

/** Persist all tasks to disk. */
export function saveTasks(tasks: Task[]): Promise<void> {
  return saveCollection<Task>(TASKS_FILE, tasks);
}

// Re-export for convenience so route modules have a single import surface.
export { PROJECTS_FILE, TASKS_FILE, DATA_DIR };
