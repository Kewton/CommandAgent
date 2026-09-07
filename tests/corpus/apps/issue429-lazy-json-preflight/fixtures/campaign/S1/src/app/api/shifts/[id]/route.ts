import { NextRequest, NextResponse } from "next/server";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";

export const dynamic = "force-dynamic";

const DATA_DIR = join(process.cwd(), "data");
const STAFF_FILE = join(DATA_DIR, "staff.json");
const SHIFTS_FILE = join(DATA_DIR, "shifts.json");

interface Shift {
  id: string;
  staffId: string;
  date: string;
  start: string;
  end: string;
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

function timeToMinutes(time: string): number {
  const [h, m] = time.split(":").map(Number);
  return h * 60 + m;
}

function is30MinAligned(time: string): boolean {
  const minutes = timeToMinutes(time);
  return minutes % 30 === 0;
}

function isValidDate(date: string): boolean {
  const regex = /^\d{4}-\d{2}-\d{2}$/;
  if (!regex.test(date)) return false;
  const d = new Date(date + "T00:00:00");
  return !isNaN(d.getTime());
}

function isValidTime(time: string): boolean {
  const regex = /^([01]\d|2[0-3]):(00|30)$/;
  return regex.test(time);
}

function hasOverlap(
  newShift: Pick<Shift, "staffId" | "date" | "start" | "end">,
  shifts: Shift[],
  excludeId: string
): boolean {
  const newStart = timeToMinutes(newShift.start);
  const newEnd = timeToMinutes(newShift.end);

  return shifts.some((s) => {
    if (s.id === excludeId) return false;
    if (s.staffId !== newShift.staffId || s.date !== newShift.date) return false;
    const existingStart = timeToMinutes(s.start);
    const existingEnd = timeToMinutes(s.end);
    return newStart < existingEnd && newEnd > existingStart;
  });
}

export async function GET(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const shifts = readShifts();
    const found = shifts.find((s) => s.id === params.id);
    if (!found) {
      return NextResponse.json({ error: "シフトが見つかりません" }, { status: 404 });
    }
    return NextResponse.json(found);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function PUT(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const body = await request.json();
    const { staffId, date, start, end, breakMinutes } = body;

    const shifts = readShifts();
    const idx = shifts.findIndex((s) => s.id === params.id);
    if (idx === -1) {
      return NextResponse.json({ error: "シフトが見つかりません" }, { status: 404 });
    }

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

    const startMinutes = timeToMinutes(start);
    let endMinutes = timeToMinutes(end);
    if (end === "24:00") endMinutes = 24 * 60;

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

    // Check overlap with other shifts (exclude current)
    if (hasOverlap({ staffId, date, start, end }, shifts, params.id)) {
      return NextResponse.json({ error: "同じスタッフのシフトが重なっています" }, { status: 400 });
    }

    shifts[idx] = {
      ...shifts[idx],
      staffId,
      date,
      start,
      end,
      breakMinutes,
    };

    writeShifts(shifts);

    return NextResponse.json(shifts[idx]);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function DELETE(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const shifts = readShifts();
    const idx = shifts.findIndex((s) => s.id === params.id);
    if (idx === -1) {
      return NextResponse.json({ error: "シフトが見つかりません" }, { status: 404 });
    }

    shifts.splice(idx, 1);
    writeShifts(shifts);

    return NextResponse.json({ success: true });
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}
