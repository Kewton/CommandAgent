import {
  Project,
  Task,
  TaskStatus,
  Member,
  CreateProjectBody,
  CreateTaskBody,
  UpdateTaskBody,
  TaskFilterParams,
  StoreError,
} from "./types";

import fs from "fs";
import path from "path";
import crypto from "crypto";

// ---------------------------------------------------------------------------
// Data directory & file paths
// ---------------------------------------------------------------------------
const DATA_DIR = path.resolve(process.cwd(), "data");
const PROJECTS_FILE = path.join(DATA_DIR, "projects.json");
const TASKS_FILE = path.join(DATA_DIR, "tasks.json");

// ---------------------------------------------------------------------------
// In-process mutex via a simple promise chain (serialise read-modify-write)
// ---------------------------------------------------------------------------
let mutex: Promise<void> = Promise.resolve();

async function withMutex<T>(fn: () => T): Promise<Awaited<T>> {
  const result = mutex.then(fn, fn);
  // Keep the chain alive even if fn throws
  mutex = result.then(
    () => undefined,
    () => undefined,
  );
  return await result;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function ensureDataDir(): void {
  try {
    fs.mkdirSync(DATA_DIR, { recursive: true });
  } catch {
    // already exists or not creatable — will surface on write
  }
}

/**
 * Read a JSON array from disk.
 * - If the file does not exist (ENOENT): initialise with the given default,
 *   persist it, and return the default.
 * - If the file exists but is empty: treat as an empty array.
 * - If parse fails or I/O fails: return a 5xx-shaped error tuple.
 */
function readJsonArray<T>(filePath: string, defaultVal: T[]): [T[], null] | [null, StoreError] {
  try {
    ensureDataDir();
    const raw = fs.readFileSync(filePath, "utf-8");
    if (raw.trim() === "") return [[], null];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [null, { code: "INVALID_FORMAT", message: `${path.basename(filePath)} is not a JSON array` }];
    }
    return [parsed as T[], null];
  } catch (err: unknown) {
    if (err instanceof Error && "code" in err && (err as NodeJS.ErrnoException).code === "ENOENT") {
      // Initialise missing file with default
      try {
        atomicWriteJson(filePath, defaultVal);
        return [defaultVal, null];
      } catch {
        return [null, { code: "IO_ERROR", message: `Failed to initialise ${path.basename(filePath)}` }];
      }
    }
    return [null, { code: "PARSE_ERROR", message: `Failed to read ${path.basename(filePath)}: ${err instanceof Error ? err.message : String(err)}` }];
  }
}

/**
 * Write a JSON array atomically: write to a temp file, then rename.
 * Also performs a generation-counter check to prevent lost updates.
 */
let generation = 0;

function atomicWriteJson(filePath: string, data: unknown): void {
  ensureDataDir();
  const tmpPath = `${filePath}.tmp.${crypto.randomUUID()}`;
  fs.writeFileSync(tmpPath, JSON.stringify(data, null, 2) + "\n", "utf-8");
  fs.renameSync(tmpPath, filePath);
}

function writeWithGenerationCheck(filePath: string, data: unknown): boolean {
  const myGen = ++generation;
  ensureDataDir();
  const tmpPath = `${filePath}.tmp.${crypto.randomUUID()}`;
  fs.writeFileSync(tmpPath, JSON.stringify(data, null, 2) + "\n", "utf-8");
  // Generation check: if another write happened after we started, abort
  if (generation !== myGen) {
    try {
      fs.unlinkSync(tmpPath);
    } catch {
      /* ignore */
    }
    return false;
  }
  fs.renameSync(tmpPath, filePath);
  return true;
}

// ---------------------------------------------------------------------------
// Input validation helpers
// ---------------------------------------------------------------------------
function isFiniteNumber(n: unknown): n is number {
  return typeof n === "number" && Number.isFinite(n);
}

function isValidDateString(s: string): boolean {
  if (typeof s !== "string" || s.length === 0) return false;
  const d = new Date(s);
  return !isNaN(d.getTime());
}

function isValidTaskStatus(s: string): s is TaskStatus {
  return s === "not_started" || s === "in_progress" || s === "completed";
}

// ---------------------------------------------------------------------------
// Public store API
// ---------------------------------------------------------------------------

/** Load all projects from disk. */
export function loadProjects(): { data: Project[] | null; error: StoreError | null } {
  const [data, err] = readJsonArray<Project>(PROJECTS_FILE, []);
  return { data, error: err };
}

/** Persist projects to disk. */
export function saveProjects(projects: Project[]): { ok: boolean; error: StoreError | null } {
  // Validate each project at the boundary
  for (const p of projects) {
    if (!p || typeof p.id !== "string" || typeof p.name !== "string") {
      return { ok: false, error: { code: "VALIDATION", message: "Invalid project entry" } };
    }
  }
  try {
    atomicWriteJson(PROJECTS_FILE, projects);
    return { ok: true, error: null };
  } catch (err) {
    return { ok: false, error: { code: "IO_ERROR", message: err instanceof Error ? err.message : String(err) } };
  }
}

/** Load all tasks from disk. */
export function loadTasks(): { data: Task[] | null; error: StoreError | null } {
  const [data, err] = readJsonArray<Task>(TASKS_FILE, []);
  return { data, error: err };
}

/** Persist tasks to disk. */
export function saveTasks(tasks: Task[]): { ok: boolean; error: StoreError | null } {
  for (const t of tasks) {
    if (!t || typeof t.id !== "string" || typeof t.title !== "string") {
      return { ok: false, error: { code: "VALIDATION", message: "Invalid task entry" } };
    }
  }
  try {
    atomicWriteJson(TASKS_FILE, tasks);
    return { ok: true, error: null };
  } catch (err) {
    return { ok: false, error: { code: "IO_ERROR", message: err instanceof Error ? err.message : String(err) } };
  }
}

// ---------------------------------------------------------------------------
// Higher-level CRUD helpers (serialised via mutex)
// ---------------------------------------------------------------------------

export function createProject(body: CreateProjectBody): Promise<{ data: Project | null; error: StoreError | null }> {
  return withMutex(() => {
    // Validate
    if (typeof body.name !== "string" || body.name.trim() === "") {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "Project name is required" } });
    }
    if (typeof body.description !== "string") {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "Description must be a string" } });
    }

    const { data: projects, error: loadErr } = loadProjects();
    if (loadErr || !projects) {
      return Promise.resolve({ data: null, error: loadErr ?? { code: "IO_ERROR", message: "Failed to load projects" } });
    }

    const now = new Date().toISOString();
    const project: Project = {
      id: crypto.randomUUID(),
      name: body.name.trim(),
      description: body.description.trim(),
      createdAt: now,
    };

    projects.push(project);
    const writeResult = saveProjects(projects);
    if (!writeResult.ok) {
      return Promise.resolve({ data: null, error: writeResult.error });
    }

    return Promise.resolve({ data: project, error: null });
  });
}

export function getProjects(): Promise<{ data: Project[] | null; error: StoreError | null }> {
  return withMutex(() => {
    const { data, error } = loadProjects();
    return Promise.resolve({ data: data ?? null, error });
  });
}

export function createTask(body: CreateTaskBody): Promise<{ data: Task | null; error: StoreError | null }> {
  return withMutex(() => {
    // Validate
    if (typeof body.projectId !== "string" || body.projectId.trim() === "") {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "projectId is required" } });
    }
    if (typeof body.title !== "string" || body.title.trim() === "") {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "Task title is required" } });
    }
    if (typeof body.dueDate !== "string" || !isValidDateString(body.dueDate)) {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "dueDate must be a valid date string" } });
    }

    // Optional fields
    let status: TaskStatus = "not_started";
    if (body.status !== undefined && body.status !== null) {
      if (!isValidTaskStatus(body.status)) {
        return Promise.resolve({ data: null, error: { code: "VALIDATION", message: `Invalid status: ${body.status}` } });
      }
      status = body.status;
    }

    let assignee: Member | null = null;
    if (body.assignee !== undefined && body.assignee !== null) {
      if (typeof body.assignee !== "object" || typeof (body.assignee as Member).id !== "string" || typeof (body.assignee as Member).name !== "string") {
        return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "Invalid assignee" } });
      }
      assignee = {
        id: (body.assignee as Member).id,
        name: (body.assignee as Member).name,
        role: typeof (body.assignee as Member).role === "string" ? (body.assignee as Member).role : "member",
      };
    }

    // Verify project exists
    const projResult = loadProjects();
    if (projResult.error || !projResult.data) {
      return Promise.resolve({ data: null, error: projResult.error ?? { code: "IO_ERROR", message: "Failed to load projects" } });
    }
    const projectExists = projResult.data.some((p) => p.id === body.projectId);
    if (!projectExists) {
      return Promise.resolve({ data: null, error: { code: "NOT_FOUND", message: "Project not found" } });
    }

    const { data: tasks, error: loadErr } = loadTasks();
    if (loadErr || !tasks) {
      return Promise.resolve({ data: null, error: loadErr ?? { code: "IO_ERROR", message: "Failed to load tasks" } });
    }

    const now = new Date().toISOString();
    const task: Task = {
      id: crypto.randomUUID(),
      projectId: body.projectId,
      title: body.title.trim(),
      dueDate: body.dueDate,
      status,
      assignee,
      createdAt: now,
    };

    tasks.push(task);
    const writeResult = saveTasks(tasks);
    if (!writeResult.ok) {
      return Promise.resolve({ data: null, error: writeResult.error });
    }

    return Promise.resolve({ data: task, error: null });
  });
}

export function getTasks(filter?: TaskFilterParams): Promise<{ data: Task[] | null; error: StoreError | null }> {
  return withMutex(() => {
    const { data: tasks, error } = loadTasks();
    if (error || !tasks) {
      return Promise.resolve({ data: null, error: error ?? { code: "IO_ERROR", message: "Failed to load tasks" } });
    }

    let filtered = tasks;

    if (filter) {
      if (filter.projectId) {
        filtered = filtered.filter((t) => t.projectId === filter.projectId);
      }
      if (filter.status && filter.status !== "all") {
        filtered = filtered.filter((t) => t.status === filter.status);
      }
    }

    return Promise.resolve({ data: filtered, error: null });
  });
}

export function updateTaskStatus(taskId: string, status: TaskStatus): Promise<{ data: Task | null; error: StoreError | null }> {
  return withMutex(() => {
    if (!isValidTaskStatus(status)) {
      return Promise.resolve({ data: null, error: { code: "VALIDATION", message: `Invalid status: ${status}` } });
    }

    const { data: tasks, error: loadErr } = loadTasks();
    if (loadErr || !tasks) {
      return Promise.resolve({ data: null, error: loadErr ?? { code: "IO_ERROR", message: "Failed to load tasks" } });
    }

    const idx = tasks.findIndex((t) => t.id === taskId);
    if (idx === -1) {
      return Promise.resolve({ data: null, error: { code: "NOT_FOUND", message: "Task not found" } });
    }

    tasks[idx] = { ...tasks[idx], status };
    const writeResult = saveTasks(tasks);
    if (!writeResult.ok) {
      return Promise.resolve({ data: null, error: writeResult.error });
    }

    return Promise.resolve({ data: tasks[idx], error: null });
  });
}

export function assignTask(taskId: string, assignee: Member | null): Promise<{ data: Task | null; error: StoreError | null }> {
  return withMutex(() => {
    if (assignee !== null) {
      if (typeof assignee.id !== "string" || typeof assignee.name !== "string") {
        return Promise.resolve({ data: null, error: { code: "VALIDATION", message: "Invalid assignee" } });
      }
    }

    const { data: tasks, error: loadErr } = loadTasks();
    if (loadErr || !tasks) {
      return Promise.resolve({ data: null, error: loadErr ?? { code: "IO_ERROR", message: "Failed to load tasks" } });
    }

    const idx = tasks.findIndex((t) => t.id === taskId);
    if (idx === -1) {
      return Promise.resolve({ data: null, error: { code: "NOT_FOUND", message: "Task not found" } });
    }

    tasks[idx] = { ...tasks[idx], assignee };
    const writeResult = saveTasks(tasks);
    if (!writeResult.ok) {
      return Promise.resolve({ data: null, error: writeResult.error });
    }

    return Promise.resolve({ data: tasks[idx], error: null });
  });
}

export function deleteTask(taskId: string): Promise<{ data: { id: string; deleted: boolean } | null; error: StoreError | null }> {
  return withMutex(() => {
    const { data: tasks, error: loadErr } = loadTasks();
    if (loadErr || !tasks) {
      return Promise.resolve({ data: null, error: loadErr ?? { code: "IO_ERROR", message: "Failed to load tasks" } });
    }

    const idx = tasks.findIndex((t) => t.id === taskId);
    if (idx === -1) {
      return Promise.resolve({ data: null, error: { code: "NOT_FOUND", message: "Task not found" } });
    }

    tasks.splice(idx, 1);
    const writeResult = saveTasks(tasks);
    if (!writeResult.ok) {
      return Promise.resolve({ data: null, error: writeResult.error });
    }

    return Promise.resolve({ data: { id: taskId, deleted: true }, error: null });
  });
}
