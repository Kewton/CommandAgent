import { NextRequest, NextResponse } from "next/server";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";
import { randomUUID } from "crypto";

export const dynamic = "force-dynamic";

const DATA_DIR = join(process.cwd(), "data");
const STAFF_FILE = join(DATA_DIR, "staff.json");
const SHIFTS_FILE = join(DATA_DIR, "shifts.json");

interface Shift {
  id: string;
  staffId: string;
  date: string; // YYYY-MM-DD
  start: string; // HH:MM
  end: string;   // HH:MM
  breakMinutes: number;
}

function readStaff(): { id: string }[] {
  if (!existsSync(STAFF_FILE)) return [];
  return JSON.parse(readFileSync(STAFF_FILE, "utf-8"));
}

function readShifts(): Shift[] {
  if (!existsSync(SHIFTS_FILE)) return [];
  return JSON.parse(readFileSync(SHIFTS_FILE, "utf-8"));
}

function writeShifts(shifts: Shift[]): void {
  writeFileSync(SHIFTS_FILE, JSON.stringify(shifts, null, 2), "utf-8");
}

// Convert HH:MM to minutes since midnight
function timeToMinutes(time: string): number {
  const [h, m] = time.split(":").map(Number);
  return h * 60 + m;
}

// Check if time is 30-min aligned
function is30MinAligned(time: string): boolean {
  const minutes = timeToMinutes(time);
  return minutes % 30 === 0;
}

// Validate date format YYYY-MM-DD
function isValidDate(date: string): boolean {
  const regex = /^\d{4}-\d{2}-\d{2}$/;
  if (!regex.test(date)) return false;
  const d = new Date(date + "T00:00:00");
  return !isNaN(d.getTime());
}

// Validate time format HH:MM
function isValidTime(time: string): boolean {
  const regex = /^([01]\d|2[0-3]):(00|30)$/;
  return regex.test(time);
}

// Check overlap: two shifts overlap if start < other.end AND end > other.start
// Adjacent (start == other.end or end == other.start) is NOT an overlap
function hasOverlap(
  newShift: Pick<Shift, "staffId" | "date" | "start" | "end">,
  shifts: Shift[]
): boolean {
  const newStart = timeToMinutes(newShift.start);
  const newEnd = timeToMinutes(newShift.end);

  return shifts.some((s) => {
    if (s.staffId !== newShift.staffId || s.date !== newShift.date) return false;
    const existingStart = timeToMinutes(s.start);
    const existingEnd = timeToMinutes(s.end);
    // Overlap if the intervals (start, end) intersect
    // Adjacent shifts (end == next.start) do NOT overlap
    return newStart < existingEnd && newEnd > existingStart;
  });
}

export async function GET() {
  try {
    const shifts = readShifts();
    return NextResponse.json(shifts);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { staffId, date, start, end, breakMinutes } = body;

    // Validate staffId exists
    const staff = readStaff();
    if (!staffId || !staff.some((s) => s.id === staffId)) {
      return NextResponse.json({ error: "有効なスタッフを選択してください" }, { status: 400 });
    }

    // Validate date
    if (!date || !isValidDate(date)) {
      return NextResponse.json({ error: "有効な日付を指定してください" }, { status: 400 });
    }

    // Validate start time
    if (!start || !isValidTime(start)) {
      return NextResponse.json({ error: "開始時刻は30分単位で指定してください（00:00-23:30）" }, { status: 400 });
    }

    // Validate end time
    if (!end || !isValidTime(end)) {
      return NextResponse.json({ error: "終了時刻は30分単位で指定してください（00:30-24:00）" }, { status: 400 });
    }

    // Handle 24:00 as end time
    const startMinutes = timeToMinutes(start);
    let endMinutes = timeToMinutes(end);
    // Allow 24:00 as valid end time
    if (end === "24:00") endMinutes = 24 * 60;

    // end must be after start
    if (endMinutes <= startMinutes) {
      return NextResponse.json({ error: "終了時刻は開始時刻より後にしてください" }, { status: 400 });
    }

    // Validate breakMinutes
    const workMinutes = endMinutes - startMinutes;
    if (breakMinutes === undefined || breakMinutes === null || typeof breakMinutes !== "number") {
      return NextResponse.json({ error: "休憩分数を指定してください" }, { status: 400 });
    }
    if (breakMinutes < 0) {
      return NextResponse.json({ error: "休憩分数は0以上で指定してください" }, { status: 400 });
    }
    if (breakMinutes >= workMinutes) {
      return NextResponse.json({ error: "休憩は勤務時間より短い必要があります" }, { status: 400 });
    }

    // Check overlap with existing shifts
    const shifts = readShifts();
    if (hasOverlap({ staffId, date, start, end }, shifts)) {
      return NextResponse.json({ error: "同じスタッフのシフトが重なっています" }, { status: 400 });
    }

    const newShift: Shift = {
      id: randomUUID(),
      staffId,
      date,
      start,
      end,
      breakMinutes,
    };

    shifts.push(newShift);
    writeShifts(shifts);

    return NextResponse.json(newShift, { status: 201 });
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}
