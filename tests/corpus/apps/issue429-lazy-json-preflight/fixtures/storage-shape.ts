import { promises as fs } from 'fs';
import path from 'path';

const DATA_DIR = path.join(process.cwd(), 'data');

async function ensureDataDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

export async function readStaff(): Promise<unknown[]> {
  const filePath = path.join(DATA_DIR, 'staff.json');
  try {
    return JSON.parse(await fs.readFile(filePath, 'utf-8'));
  } catch {
    return [];
  }
}

export async function writeStaff(staff: unknown[]): Promise<void> {
  await ensureDataDir();
  const filePath = path.join(DATA_DIR, 'staff.json');
  await fs.writeFile(filePath, JSON.stringify(staff, null, 2), 'utf-8');
}

export async function writeShifts(shifts: unknown[]): Promise<void> {
  await ensureDataDir();
  const filePath = path.join(DATA_DIR, 'shifts.json');
  await fs.writeFile(filePath, JSON.stringify(shifts, null, 2), 'utf-8');
}

export async function ensureSeeded(): Promise<void> {
  if ((await readStaff()).length === 0) {
    await writeStaff([{ id: 'synthetic-staff' }]);
    await writeShifts([{ id: 'synthetic-shift' }]);
  }
}
