import { NextResponse } from "next/server";
import {
  updateTaskStatus,
  assignTask,
  deleteTask,
} from "@/lib/store";
import { UpdateTaskBody, TaskStatus, Member } from "@/lib/types";

/**
 * PATCH /api/tasks/[id]
 * Accepts UpdateTaskBody (status change, reassignment).
 * Validates and delegates to updateTaskStatus or assignTask.
 */
export async function PATCH(
  request: Request,
  { params }: { params: { id: string } }
) {
  try {
    const body: unknown = await request.json();
    if (!body || typeof body !== "object") {
      return NextResponse.json(
        { error: "Invalid request body", details: "Body must be a JSON object." },
        { status: 400 }
      );
    }

    const { id } = params;
    if (!id || typeof id !== "string") {
      return NextResponse.json(
        { error: "Invalid task id", details: "Task id parameter is required." },
        { status: 400 }
      );
    }

    const { status, assignee } = body as Partial<UpdateTaskBody>;

    // If a status change is requested
    if (status !== undefined) {
      const validStatuses: TaskStatus[] = ["not_started", "in_progress", "completed"];
      if (typeof status !== "string" || !validStatuses.includes(status as TaskStatus)) {
        return NextResponse.json(
          {
            error: "Invalid status",
            details: "status must be one of: not_started, in_progress, completed.",
          },
          { status: 400 }
        );
      }

      const result = await updateTaskStatus(id, status as TaskStatus);
      if (result.data) {
        return NextResponse.json(
          { item: result.data },
          { status: 200 }
        );
      } else {
        if (result.error?.code !== "VALIDATION" && result.error?.code !== "NOT_FOUND") {
          return NextResponse.json({ error: result.error?.message ?? "Operation failed", details: result.error?.details }, { status: 500 });
        }
        return NextResponse.json(
          { error: result.error?.message ?? "Operation failed", details: result.error?.details },
          { status: 400 }
        );
      }
    }

    // If an assignment change is requested
    if (assignee !== undefined) {
      if (assignee !== null && (typeof assignee !== "object" || typeof assignee.id !== "string" || typeof assignee.name !== "string" || typeof assignee.role !== "string")) {
        return NextResponse.json(
          {
            error: "Invalid assignee",
            details: "assignee must be a Member object or null.",
          },
          { status: 400 }
        );
      }

      const result = await assignTask(id, assignee as Member | null);
      if (result.data) {
        return NextResponse.json(
          { item: result.data },
          { status: 200 }
        );
      } else {
        if (result.error?.code !== "VALIDATION" && result.error?.code !== "NOT_FOUND") {
          return NextResponse.json({ error: result.error?.message ?? "Operation failed", details: result.error?.details }, { status: 500 });
        }
        return NextResponse.json(
          { error: result.error?.message ?? "Operation failed", details: result.error?.details },
          { status: 400 }
        );
      }
    }

    // No valid update fields provided
    return NextResponse.json(
      {
        error: "No valid update fields",
        details: "Provide 'status' and/or 'assignee' in the request body.",
      },
      { status: 400 }
    );
  } catch (err) {
    return NextResponse.json(
      { error: "Internal Server Error", details: String(err) },
      { status: 500 }
    );
  }
}

/**
 * DELETE /api/tasks/[id]
 * Calls deleteTask and returns { item: { id } } on success.
 */
export async function DELETE(
  request: Request,
  { params }: { params: { id: string } }
) {
  try {
    const { id } = params;
    if (!id || typeof id !== "string") {
      return NextResponse.json(
        { error: "Invalid task id", details: "Task id parameter is required." },
        { status: 400 }
      );
    }

    const result = await deleteTask(id);
    if (result.data) {
      return NextResponse.json(
        { item: { id: result.data.id } },
        { status: 200 }
      );
    } else {
      if (result.error?.code !== "NOT_FOUND") {
        return NextResponse.json({ error: result.error?.message ?? "Operation failed", details: result.error?.details }, { status: 500 });
      }
      return NextResponse.json(
        { error: result.error?.message ?? "Operation failed", details: result.error?.details },
        { status: 404 }
      );
    }
  } catch (err) {
    return NextResponse.json(
      { error: "Internal Server Error", details: String(err) },
      { status: 500 }
    );
  }
}
