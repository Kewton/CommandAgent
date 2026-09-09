import { NextResponse } from "next/server";
import { getTasks, createTask, getProjects } from "@/lib/store";
import { CreateTaskBody, TaskStatus, Task } from "@/lib/types";

// GET /api/projects/:id/tasks — return tasks for a project, optionally filtered by status
export async function GET(
  request: Request,
  { params }: { params: { id: string } }
) {
  try {
    const projectId = params.id;
    const url = new URL(request.url);
    const statusParam = url.searchParams.get("status") || "";

    // Verify project exists
    const projectsResult = await getProjects();
    if (projectsResult.error) {
      return NextResponse.json(projectsResult.error, { status: 500 });
    }
    const projects = projectsResult.data ?? [];
    const project = projects.find((p) => p.id === projectId);
    if (!project) {
      return NextResponse.json(
        { error: "Project not found", details: { projectId } },
        { status: 404 }
      );
    }

    const tasksResult = await getTasks();
    if (tasksResult.error) {
      return NextResponse.json(tasksResult.error, { status: 500 });
    }

    let tasks = (tasksResult.data ?? []).filter((t) => t.projectId === projectId);

    // Filter by status if provided
    if (statusParam && statusParam !== "all") {
      const validStatuses: TaskStatus[] = [
        "not_started",
        "in_progress",
        "completed",
      ];
      const status = statusParam as string;
      if (validStatuses.includes(status as TaskStatus)) {
        tasks = tasks.filter((t) => t.status === (status as TaskStatus));
      }
    }

    return NextResponse.json({ items: tasks }, { status: 200 });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Internal server error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}

// POST /api/projects/:id/tasks — create a new task for a project
export async function POST(
  request: Request,
  { params }: { params: { id: string } }
) {
  try {
    const projectId = params.id;
    const body: unknown = await request.json();

    // Verify project exists
    const projectsResult = await getProjects();
    if (projectsResult.error) {
      return NextResponse.json(projectsResult.error, { status: 500 });
    }
    const projects = projectsResult.data ?? [];
    const project = projects.find((p) => p.id === projectId);
    if (!project) {
      return NextResponse.json(
        { error: "Project not found", details: { projectId } },
        { status: 404 }
      );
    }

    // Validate body
    if (
      !body ||
      typeof body !== "object" ||
      typeof (body as Record<string, unknown>).title !== "string" ||
      typeof (body as Record<string, unknown>).dueDate !== "string"
    ) {
      return NextResponse.json(
        { error: "Invalid request body", details: { projectId } },
        { status: 400 }
      );
    }

    const dueDateStr = (body as Record<string, string>).dueDate;
    const dueDateObj = new Date(dueDateStr);
    if (isNaN(dueDateObj.getTime())) {
      return NextResponse.json(
        { error: "Invalid dueDate", details: { dueDate: dueDateStr } },
        { status: 400 }
      );
    }

    const validStatuses: TaskStatus[] = ["not_started", "in_progress", "completed"];
    const statusParam =
      (body as Record<string, string>).status ?? "not_started";
    if (!validStatuses.includes(statusParam as TaskStatus)) {
      return NextResponse.json(
        { error: "Invalid status", details: { status: statusParam } },
        { status: 400 }
      );
    }

    // Build assignee if provided
    let assignee:
      | { id: string; name: string; role?: string }
      | undefined = undefined;
    if (
      typeof (body as Record<string, unknown>).assignee === "object" &&
      (body as Record<string, unknown>).assignee !== null
    ) {
      const a = (body as Record<string, unknown>).assignee as Record<
        string,
        unknown
      >;
      if (typeof a.id === "string" && typeof a.name === "string") {
        assignee = {
          id: a.id,
          name: a.name,
          role: typeof a.role === "string" ? a.role : "member",
        };
      }
    }

    const createBody: CreateTaskBody = {
      projectId,
      title: (body as Record<string, string>).title,
      dueDate: dueDateObj.toISOString(),
      status: statusParam as TaskStatus,
      assignee,
    };

    const result = await createTask(createBody);
    if (result.error) {
      return NextResponse.json(result.error, { status: 500 });
    }

    return NextResponse.json({ item: result.data }, { status: 201 });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Internal server error";
    return NextResponse.json({ error: message }, { status: 500 });
  }
}
