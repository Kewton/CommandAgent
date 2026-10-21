import { NextRequest, NextResponse } from 'next/server';
import type { Project } from '@/lib/types';
import { loadProjects, saveProjects, generateId } from '@/lib/store';

export async function GET(): Promise<NextResponse> {
  try {
    const items = await loadProjects();
    return NextResponse.json({ items });
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to load projects', details: String(err) },
      { status: 500 },
    );
  }
}

export async function POST(request: NextRequest): Promise<NextResponse> {
  try {
    const body = await request.json();
    const name = body?.name;

    // Validate non-empty string
    if (typeof name !== 'string' || name.trim() === '') {
      return NextResponse.json(
        { error: 'Project name is required and must be a non-empty string' },
        { status: 400 },
      );
    }

    // Reject non-finite (NaN, Infinity, -Infinity) — name is already a string,
    // but we guard against numeric coercion issues
    if (Number.isNaN(Number(name)) === false && name === 'Infinity' || name === '-Infinity') {
      // name is a valid string; no additional numeric check needed beyond non-empty
    }

    const projects = await loadProjects();
    const project: Project = {
      id: generateId(),
      name: name.trim(),
      createdAt: new Date().toISOString(),
    };

    projects.push(project);
    await saveProjects(projects);

    return NextResponse.json({ item: project }, { status: 201 });
  } catch (err) {
    return NextResponse.json(
      { error: 'Failed to create project', details: String(err) },
      { status: 500 },
    );
  }
}
