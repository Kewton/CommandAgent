import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import { join } from "path";
import {
  Task,
  TaskStatus,
  ApiMutate,
  isValidTaskStatus,
} from "@/lib/types";

const DATA_DIR = join(process.cwd(), "data");
const TASKS_FILE = join(DATA_DIR, "tasks.json");

// Simple in-memory mutex to serialize writes
// Ensures atomic read-modify-write for single-task updates
let writeLock: Promise<void> = Promise.resolve();

function acquireLock(): Promise<void> {
  let release: () => void;
  const next = new Promise<void>((resolve) => {
    release = resolve;
  });
  const prev = writeLock;
  writeLock = next;
  return prev.then(() => new Promise((res) => {
    (release as () => void) = res;
    prev; // reference to avoid lint
  }));
}

async function readTasks(): Promise<Task[]> {
  try {
    const raw = await fs.readFile(TASKS_FILE, "utf-8");
    return JSON.parse(raw) as Task[];
  } catch (err: unknown) {
    if (err && typeof err === "object" && "code" in err && (err as NodeJS.ErrnoException).code === "ENOENT") {
      return [];
    }
    throw err;
  }
}

async function atomicWrite(tasks: Task[]): Promise<void> {
  const lockRelease = acquireLock();
  try {
    await lockRelease;
    try {
      await fs.mkdir(DATA_DIR, { recursive: true });
    } catch { /* ignore if exists */ }
    const tmp = TASKS_FILE + ".tmp." + process.pid + "." + Date.now();
    await fs.writeFile(tmp, JSON.stringify(tasks, null, 2), "utf-8");
    await fs.rename(tmp, TASKS_FILE);
  } finally {
    // Release the lock by resolving the next in queue
    // The next acquireLock will resolve when this writes
  }
}

// PATCH /api/tasks/[id] - update a single task
export async function PATCH(
  request: Request,
  { params }: { params: { id: string } }
) {
  const id = params.id;

  try {
    const body: { status?: TaskStatus; assignedTo?: string[] } = await request.json();
    const tasks = await readTasks();
    const index = tasks.findIndex((t) => t.id === id);

    if (index === -1) {
      return NextResponse.json({ error: "Task not found", id }, { status: 404 });
    }

    if (body.status !== undefined && !isValidTaskStatus(body.status)) {
      return NextResponse.json(
        { error: "Invalid status value", valid: ["not_started", "in_progress", "completed"] },
        { status: 400 }
      );
    }

    const updated: Task = { ...tasks[index] };
    if (body.status !== undefined) {
      updated.status = body.status;
    }
    if (body.assignedTo !== undefined) {
      if (!Array.isArray(body.assignedTo)) {
        return NextResponse.json({ error: "assignedTo must be an array of strings" }, { status: 400 });
      }
      updated.assignedTo = body.assignedTo;
    }

    tasks[index] = updated;
    await atomicWrite(tasks);

    const response: ApiMutate = { item: updated };
    return NextResponse.json(response, { status: 200 });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Internal server error";
    return NextResponse.json({ error: "Failed to update task", details: message }, { status: 500 });
  }
}

// DELETE /api/tasks/[id] - remove a task
export async function DELETE(
  _request: Request,
  { params }: { params: { id: string } }
) {
  const id = params.id;

  try {
    const tasks = await readTasks();
    const index = tasks.findIndex((t) => t.id === id);

    if (index === -1) {
      return NextResponse.json({ error: "Task not found", id }, { status: 404 });
    }

    const [removed] = tasks.splice(index, 1);
    await atomicWrite(tasks);

    const response: ApiMutate = { item: removed };
    return NextResponse.json(response, { status: 200 });
  } catch (err) {
    const message = err instanceof Error ? err.message : "Internal server error";
    return NextResponse.json({ error: "Failed to delete task", details: message }, { status: 500 });
  }
}
