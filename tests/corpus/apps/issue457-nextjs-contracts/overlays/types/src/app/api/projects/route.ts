import { NextResponse } from "next/server";
import { getProjects, createProject } from "@/lib/store";

export const dynamic = "force-dynamic";

// HTTP errors are strings; structured StoreError stays inside the store.
export async function GET() {
  try {
    const result = await getProjects();
    if (result.error) {
      return NextResponse.json({ error: result.error.message, details: result.error.details }, { status: 500 });
    }
    return NextResponse.json({ items: result.data ?? [] }, { status: 200 });
  } catch (err) {
    return NextResponse.json({ error: "Failed to fetch projects", details: err instanceof Error ? err.message : String(err) }, { status: 500 });
  }
}

export async function POST(request: Request) {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return NextResponse.json({ error: "Request body is not valid JSON" }, { status: 400 });
  }
  if (!body || typeof body !== "object" || !("name" in body) || typeof body.name !== "string" || !body.name.trim()) {
    return NextResponse.json({ error: "Project name is required and must be a non-empty string" }, { status: 400 });
  }
  if (!("description" in body) || typeof body.description !== "string") {
    return NextResponse.json({ error: "Description must be a string" }, { status: 400 });
  }
  try {
    const result = await createProject({ name: body.name, description: body.description });
    if (result.error) {
      return NextResponse.json({ error: result.error.message, details: result.error.details }, { status: result.error.status ?? 500 });
    }
    return NextResponse.json({ item: result.data }, { status: 201 });
  } catch (err) {
    return NextResponse.json({ error: "Failed to create project", details: err instanceof Error ? err.message : String(err) }, { status: 500 });
  }
}
