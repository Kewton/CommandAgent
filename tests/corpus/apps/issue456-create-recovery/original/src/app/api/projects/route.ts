import { NextResponse } from "next/server";
import { getProjects, createProject } from "@/lib/store";
import { Project, CreateProjectBody, ApiResponse } from "@/lib/types";

// GET /api/projects → returns all projects
export async function GET() {
  try {
    const result = await getProjects();
    if (result.error) {
      return NextResponse.json(result.error as ApiResponse<Project[]>, {
        status: 500,
      });
    }
    return NextResponse.json({ items: result.data ?? [] }, { status: 200 });
  } catch (err) {
    return NextResponse.json(
      {
        error: {
          code: "INTERNAL_ERROR",
          message: "Failed to fetch projects",
          details: err instanceof Error ? err.message : String(err),
        },
      } as ApiResponse<Project[]>,
      { status: 500 },
    );
  }
}

// POST /api/projects → creates a new project
export async function POST(request: Request) {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return NextResponse.json(
      {
        error: {
          code: "INVALID_JSON",
          message: "Request body is not valid JSON",
        },
      } as ApiResponse<Project>,
      { status: 400 },
    );
  }

  // Basic validation
  const candidate = body as Partial<CreateProjectBody>;
  if (!candidate || typeof candidate !== "object") {
    return NextResponse.json(
      {
        error: {
          code: "INVALID_BODY",
          message: "Request body must be an object",
        },
      } as ApiResponse<Project>,
      { status: 400 },
    );
  }

  if (typeof candidate.name !== "string" || candidate.name.trim().length === 0) {
    return NextResponse.json(
      {
        error: {
          code: "VALIDATION_ERROR",
          message: "Project name is required and must be a non-empty string",
        },
      } as ApiResponse<Project>,
      { status: 400 },
    );
  }

  if (typeof candidate.description !== "string") {
    return NextResponse.json(
      {
        error: {
          code: "VALIDATION_ERROR",
          message: "Description must be a string",
        },
      } as ApiResponse<Project>,
      { status: 400 },
    );
  }

  try {
    const result = await createProject(candidate as CreateProjectBody);
    if (result.error) {
      return NextResponse.json(result.error as ApiResponse<Project>, {
        status: result.error.status ?? 500,
      });
    }
    return NextResponse.json({ item: result.data! }, { status: 201 });
  } catch (err) {
    return NextResponse.json(
      {
        error: {
          code: "INTERNAL_ERROR",
          message: "Failed to create project",
          details: err instanceof Error ? err.message : String(err),
        },
      } as ApiResponse<Project>,
      { status: 500 },
    );
  }
}
