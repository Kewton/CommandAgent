import { NextRequest, NextResponse } from 'next/server';
import type { Task, TaskStatus } from '@/lib/types';
import { TASK_STATUSES, TEAM_MEMBERS } from '@/lib/types';
import { loadProjects, loadTasks, saveTasks, generateId } from '@/lib/store';

const VALID_STATUSES: TaskStatus[] = [...TASK_STATUSES];

function isValidStatus(status: unknown): status is TaskStatus {
  return typeof status === 'string' && VALID_STATUSES.includes(status as TaskStatus);
}

// ---- GET: list tasks with optional filters ----
export async function GET(request: NextRequest): Promise<NextResponse> {
  try {
    const { searchParams } = new URL(request.url);
    const projectId = searchParams.get('projectId') ?? undefined;
    const status = searchParams.get('status') ?? undefined;

    let tasks = await loadTasks();

    // Filter by projectId
    if (projectId) {
      tasks = tasks.filter((t) => t.projectId === projectId);
    }

    // Filter by status
    if (status) {
      if (!isValidStatus(status)) {
        return NextResponse.json(
          { error: `Invalid status filter. Must be one of: ${VALID_STATUSES.join(', ')}` },
          { status: 400 },
        );
      }
      tasks = tasks.filter((t) => t.status === status);
    }

    return NextResponse.json({ items: tasks });
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to load tasks', details: String(err) },
      { status: 500 },
    );
  }
}

// ---- POST: create a new task ----
export async function POST(request: NextRequest): Promise<NextResponse> {
  try {
    const body: unknown = await request.json();

    // Validate required fields
    if (body === null || typeof body !== 'object') {
      return NextResponse.json(
        { error: 'Request body must be a JSON object' },
        { status: 400 },
      );
    }

    const { projectId, title, dueDate, assigneeId, status } = body as Record<string, unknown>;

    // projectId: non-empty string
    if (typeof projectId !== 'string' || projectId.trim() === '') {
      return NextResponse.json(
        { error: 'projectId is required and must be a non-empty string' },
        { status: 400 },
      );
    }

    // title: non-empty string
    if (typeof title !== 'string' || title.trim() === '') {
      return NextResponse.json(
        { error: 'title is required and must be a non-empty string' },
        { status: 400 },
      );
    }

    // dueDate: must be a valid ISO date string
    if (typeof dueDate !== 'string' || dueDate.trim() === '') {
      return NextResponse.json(
        { error: 'dueDate is required and must be a non-empty string (ISO date)' },
        { status: 400 },
      );
    }

    const dateObj = new Date(dueDate);
    if (Number.isNaN(dateObj.getTime())) {
      return NextResponse.json(
        { error: 'dueDate must be a valid date', details: `Received: ${dueDate}` },
        { status: 400 },
      );
    }

    // assigneeId: must match a known TeamMember
    if (typeof assigneeId !== 'string' || assigneeId.trim() === '') {
      return NextResponse.json(
        { error: 'assigneeId is required and must be a non-empty string' },
        { status: 400 },
      );
    }

    const memberExists = TEAM_MEMBERS.some((m) => m.id === assigneeId);
    if (!memberExists) {
      return NextResponse.json(
        {
          error: 'assigneeId must reference a valid team member',
          details: `Available member IDs: ${TEAM_MEMBERS.map((m) => m.id).join(', ')}`,
        },
        { status: 400 },
      );
    }

    // status: must be a valid TaskStatus
    if (!isValidStatus(status)) {
      return NextResponse.json(
        {
          error: `status is required and must be one of: ${VALID_STATUSES.join(', ')}`,
        },
        { status: 400 },
      );
    }

    // Ensure projectId exists
    const projects = await loadProjects();
    const projectExists = projects.some((p) => p.id === projectId);
    if (!projectExists) {
      return NextResponse.json(
        { error: 'projectId must reference an existing project' },
        { status: 400 },
      );
    }

    // Create task
    const tasks = await loadTasks();
    const task: Task = {
      id: generateId(),
      projectId: projectId.trim(),
      title: title.trim(),
      dueDate: dateObj.toISOString(),
      assigneeId: assigneeId.trim(),
      status,
      createdAt: new Date().toISOString(),
    };

    tasks.push(task);
    await saveTasks(tasks);

    return NextResponse.json({ item: task }, { status: 201 });
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to create task', details: String(err) },
      { status: 500 },
    );
  }
}

// ---- PUT: update a task's status ----
export async function PUT(request: NextRequest): Promise<NextResponse> {
  try {
    const body: unknown = await request.json();

    if (body === null || typeof body !== 'object') {
      return NextResponse.json(
        { error: 'Request body must be a JSON object' },
        { status: 400 },
      );
    }

    const { id, status } = body as Record<string, unknown>;

    if (typeof id !== 'string' || id.trim() === '') {
      return NextResponse.json(
        { error: 'id is required and must be a non-empty string' },
        { status: 400 },
      );
    }

    if (!isValidStatus(status)) {
      return NextResponse.json(
        {
          error: `status must be one of: ${VALID_STATUSES.join(', ')}`,
        },
        { status: 400 },
      );
    }

    const tasks = await loadTasks();
    const index = tasks.findIndex((t) => t.id === id);

    if (index === -1) {
      return NextResponse.json(
        { error: 'Task not found', details: `No task with id: ${id}` },
        { status: 404 },
      );
    }

    tasks[index] = { ...tasks[index], status };
    await saveTasks(tasks);

    return NextResponse.json({ item: tasks[index] });
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to update task', details: String(err) },
      { status: 500 },
    );
  }
}
