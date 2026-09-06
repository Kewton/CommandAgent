import { ensureSeeded, readStaff } from '../../../lib/storage.js';

export async function GET() {
  await ensureSeeded();
  return { status: 200, body: await readStaff() };
}
