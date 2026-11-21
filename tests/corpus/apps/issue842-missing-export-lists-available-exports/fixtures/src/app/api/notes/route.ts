import { NextResponse } from 'next/server';
import { listNotes, createNote } from '@/lib/notes';
import { NoteError } from '@/lib/types';

export async function GET() {
  try {
    const notes = await listNotes();
    return NextResponse.json(notes);
  } catch (error) {
    const noteError = error as NoteError;
    return NextResponse.json(
      { error: noteError.error || 'Failed to fetch notes' },
      { status: 500 }
    );
  }
}

export async function POST(request: Request) {
  try {
    const body = await request.json();
    if (!body.title) {
      return NextResponse.json(
        { error: 'Title is required' },
        { status: 400 }
      );
    }
    const note = await createNote(body);
    return NextResponse.json(note, { status: 201 });
  } catch (error) {
    const noteError = error as NoteError;
    return NextResponse.json(
      { error: noteError.error || 'Failed to create note' },
      { status: 500 }
    );
  }
}
