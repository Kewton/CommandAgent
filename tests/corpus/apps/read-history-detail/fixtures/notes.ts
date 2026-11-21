import fs from 'fs/promises';
import path from 'path';
import { Note, NoteError } from './types';

const NOTES_FILE = path.join(process.cwd(), 'notes.json');
const TEMP_FILE = path.join(process.cwd(), 'notes.json.tmp');

async function readNotes(): Promise<Note[]> {
  try {
    const data = await fs.readFile(NOTES_FILE, 'utf8');
    return JSON.parse(data);
  } catch (error: any) {
    if (error.code === 'ENOENT') {
      return [];
    }
    throw new Error(`Failed to read notes: ${error.message}`);
  }
}

async function writeNotes(notes: Note[]): Promise<void> {
  try {
    const data = JSON.stringify(notes, null, 2);
    await fs.writeFile(TEMP_FILE, data, 'utf8');
    await fs.rename(TEMP_FILE, NOTES_FILE);
  } catch (error: any) {
    throw new Error(`Failed to write notes: ${error.message}`);
  }
}

export async function getNotes(): Promise<{ data?: Note[]; error?: NoteError }> {
  try {
    const notes = await readNotes();
    return { data: notes };
  } catch (error: any) {
    return {
      error: {
        error: 'Internal Server Error',
        details: error.message,
      },
    };
  }
}

export async function saveNote(note: Note): Promise<{ data?: Note; error?: NoteError }> {
  try {
    const notes = await readNotes();
    const index = notes.findIndex((n) => n.id === note.id);
    
    if (index !== -1) {
      notes[index] = note;
    } else {
      notes.push(note);
    }
    
    await writeNotes(notes);
    return { data: note };
  } catch (error: any) {
    return {
      error: {
        error: 'Internal Server Error',
        details: error.message,
      },
    };
  }
}

export async function deleteNote(id: string): Promise<{ error?: NoteError }> {
  try {
    const notes = await readNotes();
    const filteredNotes = notes.filter((n) => n.id !== id);
    await writeNotes(filteredNotes);
    return {};
  } catch (error: any) {
    return {
      error: {
        error: 'Internal Server Error',
        details: error.message,
      },
    };
  }
}
