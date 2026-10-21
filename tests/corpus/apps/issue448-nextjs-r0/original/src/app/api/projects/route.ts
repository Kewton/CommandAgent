import { NextResponse } from "next/server";
import { promises as fs } from "fs";
import { join } from "path";
import {
  Project,
  CreateProjectInput,
  ApiListProjects,
  ApiMutate,
  generateId,
} from "@/lib/types";

// Simple in-memory mutex to serialize writes and prevent lost updates
// Shared writeLock ensures atomic read-modify-write across projects
let writeLock: Promise<void> = Promise.resolve();

function acquireLock(): Promise<void> {
  let release: () => void;
  const next = new Promise<void>((resolve) => {
    release = resolve;
  });
  const prev = writeLock;
  writeLock = next;
  return prev.then(() => release!);
}

const DATA_DIR = join(process.cwd(), "data");
const PROJECTS_FILE = join(DATA_DIR, "projects.json");

async function ensureDataDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

async function readProjects(): Promise<Project[]> {
  try {
    const raw = await fs.readFile(PROJECTS_FILE, "utf-8");
    return JSON.parse(raw) as Project[];
  } catch (err: unknown) {
    if (err instanceof Error && "code" in err && (err as NodeJS.ErrnoException).code === "ENOENT") {
      return [];
    }
    throw err;
  }
}

async function writeProjectsAtomically(projects: Project[]): Promise<void> {
  const release = await acquireLock();
  try {
    await ensureDataDir();
    const tmp = PROJECTS_FILE + ".tmp";
    await fs.writeFile(tmp, JSON.stringify(projects, null, 2), "utf-8");
    await fs.rename(tmp, PROJECTS_FILE);
  } finally {
    release();
  }
}

export async function GET() {
  try {
    const projects = await readProjects();
    const body: ApiListProjects = { projects };
    return NextResponse.json(body, { status: 200 });
  } catch (err: unknown) {
    const details = err instanceof Error ? err.message : String(err);
    return NextResponse.json(
      { error: "Internal server error", details },
      { status: 500 }
    );
  }
}

export async function POST(request: Request) {
  try {
    const body: unknown = await request.json();
    const input = body as Partial<CreateProjectInput>;

    if (!input || typeof input.name !== "string" || input.name.trim() === "") {
      return NextResponse.json(
        { error: "Validation error", details: "Project name must be non-empty." },
        { status: 400 }
      );
    }

    const members = Array.isArray(input.members) ? input.members : [];

    const project: Project = {
      id: generateId(),
      name: input.name.trim(),
      tasks: [],
      members,
    };

    const projects = await readProjects();
    projects.push(project);
    await writeProjectsAtomically(projects);

    const response: ApiMutate = { item: project };
    return NextResponse.json(response, { status: 201 });
  } catch (err: unknown) {
    const details = err instanceof Error ? err.message : String(err);
    return NextResponse.json(
      { error: "Internal server error", details },
      { status: 500 }
    );
  }
}
