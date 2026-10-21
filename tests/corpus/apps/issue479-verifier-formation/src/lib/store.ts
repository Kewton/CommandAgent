import { promises as fs } from "fs";
import path from "path";
import type { Project, Task, TeamMember } from "./types";

const DATA_DIR = path.join(process.cwd(), "data");
const PROJECTS_FILE = path.join(DATA_DIR, "projects.json");
const TASKS_FILE = path.join(DATA_DIR, "tasks.json");
const MEMBERS_FILE = path.join(DATA_DIR, "members.json");

/**
 * Ensure the data directory exists. Creates it on ENOENT only.
 */
async function ensureDataDir(): Promise<void> {
  try {
    await fs.mkdir(DATA_DIR, { recursive: true });
  } catch (err: any) {
    if (err.code === "EEXIST") return;
    throw err;
  }
}

/**
 * Atomic write: write to a temp file then rename.
 * Prevents partial writes from corrupting the target.
 */
async function atomicWrite(
  filePath: string,
  data: unknown,
): Promise<void> {
  const tmpPath = `${filePath}.${process.pid}.${Date.now()}.tmp`;
  const content = JSON.stringify(data, null, 2);
  await fs.writeFile(tmpPath, content, "utf-8");
  await fs.rename(tmpPath, filePath);
}

/**
 * Read JSON from a file. Returns defaultValue on ENOENT.
 * Throws on parse errors so the caller can return 5xx without overwriting.
 */
async function readJson<T>(filePath: string, defaultValue: T): Promise<T> {
  try {
    const raw = await fs.readFile(filePath, "utf-8");
    return JSON.parse(raw) as T;
  } catch (err: any) {
    if (err.code === "ENOENT") return defaultValue;
    // Parse error or I/O failure — re-throw to let caller return 5xx
    throw err;
  }
}

// ─── Projects ────────────────────────────────────────────────────────────────

export async function loadProjects(): Promise<Project[]> {
  await ensureDataDir();
  return readJson<Project[]>(PROJECTS_FILE, []);
}

export async function saveProjects(projects: Project[]): Promise<void> {
  await ensureDataDir();
  await atomicWrite(PROJECTS_FILE, projects);
}

// ─── Tasks ───────────────────────────────────────────────────────────────────

export async function loadTasks(): Promise<Task[]> {
  await ensureDataDir();
  return readJson<Task[]>(TASKS_FILE, []);
}

export async function saveTasks(tasks: Task[]): Promise<void> {
  await ensureDataDir();
  await atomicWrite(TASKS_FILE, tasks);
}

// ─── Members ─────────────────────────────────────────────────────────────────

export async function loadMembers(): Promise<TeamMember[]> {
  await ensureDataDir();
  return readJson<TeamMember[]>(MEMBERS_FILE, []);
}

export async function saveMembers(members: TeamMember[]): Promise<void> {
  await ensureDataDir();
  await atomicWrite(MEMBERS_FILE, members);
}
