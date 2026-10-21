import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import { join } from "path";
import {
  Task,
  Project,
  CreateTaskInput,
  ApiListTasks,
  ApiMutate,
  TaskStatus,
  generateId,
  validateDueDate,
} from "@/lib/types";

// Simple in-memory mutex to serialize writes and prevent lost updates
// Shared writeLock ensures atomic read-modify-write across tasks
let writeLock: Promise<void> = Promise.resolve();
function acquireLock(): Promise<void> {
  let release: () => void;
  const next = new Promise<void>((resolve) => {
    release = resolve;
  });
  const prev = writeLock;
  writeLock = next;
  return prev.then(() => new Promise<void>((resolve) => { release = resolve; }));
}

const DATA_DIR = join(process.cwd(), "data");
const PROJECTS_FILE = join(DATA_DIR, "projects.json");
const TASKS_FILE = join(DATA_DIR, "tasks.json");

function ensureDir() {
  fs.mkdir(DATA_DIR, { recursive: true }).catch(() => {});
}

async function loadProjects(): Promise<Project[]> {
  ensureDir();
  try {
    const raw = await fs.readFile(PROJECTS_FILE, "utf-8");
    return JSON.parse(raw) as Project[];
  } catch (e: any) {
    if (e?.code === "ENOENT") return [];
    throw e;
  }
}

async function loadTasks(): Promise<Task[]> {
  ensureDir();
  try {
    const raw = await fs.readFile(TASKS_FILE, "utf-8");
    return JSON.parse(raw) as Task[];
  } catch (e: any) {
    if (e?.code === "ENOENT") return [];
    throw e;
  }
}

async function writeAtomic(file: string, data: unknown) {
  ensureDir();
  const tmp = file + ".tmp." + process.pid + "." + Date.now();
  await fs.writeFile(tmp, JSON.stringify(data, null, 2), "utf-8");
  await fs.rename(tmp, file);
}

const VALID_STATUSES: TaskStatus[] = ["not_started", "in_progress", "completed"];

export async function GET(request: Request) {
  try {
    const url = new URL(request.url);
    const statusParam = url.searchParams.get("status") || undefined;
    let filteredStatus: TaskStatus | undefined;
    if (statusParam) {
      if (VALID_STATUSES.includes(statusParam as TaskStatus)) {
        filteredStatus = statusParam as TaskStatus;
      }
    }

    // Load tasks from the tasks file
    const tasks = await loadTasks();

    // Also check tasks embedded in projects
    const projects = await loadProjects();
    const embeddedTasks: Task[] = [];
    for (const p of projects) {
      if (Array.isArray(p.tasks)) {
        embeddedTasks.push(...p.tasks);
      }
    }

    let allTasks = [...tasks, ...embeddedTasks];

    // Apply status filter if provided
    if (filteredStatus) {
      allTasks = allTasks.filter((t) => t.status === filteredStatus);
    }

    const response: ApiListTasks = {
      tasks: allTasks,
      filters: filteredStatus ? { status: filteredStatus } : {},
    };

    return NextResponse.json(response);
  } catch (error) {
    if (error instanceof SyntaxError) {
      return NextResponse.json(
        { error: "ParseError", details: "Failed to parse stored data" },
        { status: 500 }
      );
    }
    return NextResponse.json(
      { error: "InternalError", details: String(error) },
      { status: 500 }
    );
  }
}

export async function POST(request: Request) {
  try {
    const body = await request.json();
    const { projectId, title, dueDate, assignedTo } = body as CreateTaskInput;

    // Validate required fields
    if (!title || typeof title !== "string" || title.trim() === "") {
      return NextResponse.json(
        { error: "ValidationError", details: "title is required" },
        { status: 400 }
      );
    }

    if (!dueDate || !validateDueDate(dueDate)) {
      return NextResponse.json(
        { error: "ValidationError", details: "dueDate must be a valid date" },
        { status: 400 }
      );
    }

    if (assignedTo && !Array.isArray(assignedTo)) {
      return NextResponse.json(
        { error: "ValidationError", details: "assignedTo must be an array" },
        { status: 400 }
      );
    }

    // Acquire write lock to serialize
    const release = await acquireLock();

    try {
      const tasks = await loadTasks();

      const newTask: Task = {
        id: generateId(),
        title: title.trim(),
        dueDate,
        status: "not_started",
        assignedTo: Array.isArray(assignedTo) ? assignedTo : [],
      };

      // Attach projectId for traceability
      (newTask as any).projectId = projectId || undefined;

      tasks.push(newTask);
      await writeAtomic(TASKS_FILE, tasks);

      const response: ApiMutate = { item: newTask };
      return NextResponse.json(response, { status: 201 });
    } finally {
      release();
    }
  } catch (error) {
    if (error instanceof SyntaxError) {
      return NextResponse.json(
        { error: "ParseError", details: "Invalid JSON body" },
        { status: 400 }
      );
    }
    return NextResponse.json(
      { error: "InternalError", details: String(error) },
      { status: 500 }
    );
  }
}
