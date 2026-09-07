import { NextResponse } from "next/server";
import { getStaff, getShifts, saveShifts } from "@/lib/store";
import type { Shift } from "@/lib/types";

// Validate a candidate shift against business rules.
function validateShift(candidate: Shift, all: Shift[], editingId?: string): string | null {
  if (!candidate.staffId || !candidate.date || !candidate.startTime || !candidate.endTime) {
    return "すべての項目を入力してください。";
  }

  const timeRegex = /^([01]\d|2[0-3]):([0-5]\d)$/;
  if (!timeRegex.test(candidate.startTime) || !timeRegex.test(candidate.endTime)) {
    return "時刻は HH:MM の形式で入力してください。";
  }

  const [sh, sm] = candidate.startTime.split(":").map(Number);
  const [eh, em] = candidate.endTime.split(":").map(Number);
  const startMinutes = sh * 60 + sm;
  const endMinutes = eh * 60 + em;

  if (sm % 30 !== 0 || em % 30 !== 0) {
    return "開始時刻・終了時刻は30分単位で入力してください。";
  }

  if (endMinutes <= startMinutes) {
    return "終了時刻は開始時刻より後である必要があります。";
  }

  const workingMinutes = endMinutes - startMinutes;
  const breakMinutes = candidate.breakMinutes ?? 0;
  if (typeof breakMinutes !== "number" || breakMinutes < 0) {
    return "休憩時間は0分以上の数値で入力してください。";
  }
  if (breakMinutes > workingMinutes) {
    return "休憩時間は勤務時間より短くなければなりません。";
  }
  if (breakMinutes % 30 !== 0) {
    return "休憩時間は30分単位で入力してください。";
  }

  const sameDayShifts = all.filter(
    (s) => s.staffId === candidate.staffId && s.date === candidate.date && s.id !== editingId,
  );
  for (const other of sameDayShifts) {
    const [osh, osm] = other.startTime.split(":").map(Number);
    const [oeh, oem] = other.endTime.split(":").map(Number);
    const otherStart = osh * 60 + osm;
    const otherEnd = oeh * 60 + oem;
    if (startMinutes < otherEnd && otherStart < endMinutes) {
      return "同じスタッフのシフトが重複しています。";
    }
  }

  return null;
}

// PUT /api/shifts/:id — update an existing shift with the same validation.
export async function PUT(
  request: Request,
  { params }: { params: { id: string } },
) {
  try {
    const { id } = params;
    const body = await request.json();
    const staff = getStaff();
    const shifts = getShifts();
    const index = shifts.findIndex((s) => s.id === id);

    if (index === -1) {
      return NextResponse.json(
        { error: "シフトが見つかりません。" },
        { status: 404 },
      );
    }

    const existing = shifts[index];
    const candidate: Shift = {
      id: existing.id,
      staffId: body?.staffId !== undefined ? String(body.staffId) : existing.staffId,
      date: body?.date !== undefined ? String(body.date) : existing.date,
      startTime: body?.startTime !== undefined ? String(body.startTime) : existing.startTime,
      endTime: body?.endTime !== undefined ? String(body.endTime) : existing.endTime,
      breakMinutes:
        body?.breakMinutes !== undefined
          ? typeof body.breakMinutes === "number"
            ? body.breakMinutes
            : 0
          : existing.breakMinutes,
    };

    if (!staff.some((s) => s.id === candidate.staffId)) {
      return NextResponse.json(
        { error: "有効なスタッフを選択してください。" },
        { status: 400 },
      );
    }

    const validationError = validateShift(candidate, shifts, candidate.id);
    if (validationError) {
      return NextResponse.json({ error: validationError }, { status: 400 });
    }

    shifts[index] = candidate;
    saveShifts(shifts);

    return NextResponse.json({ shift: candidate });
  } catch (error) {
    return NextResponse.json(
      { error: "シフトの更新に失敗しました。" },
      { status: 500 },
    );
  }
}

// DELETE /api/shifts/:id — remove a shift.
export async function DELETE(
  _request: Request,
  { params }: { params: { id: string } },
) {
  try {
    const { id } = params;
    const shifts = getShifts();
    const index = shifts.findIndex((s) => s.id === id);

    if (index === -1) {
      return NextResponse.json(
        { error: "シフトが見つかりません。" },
        { status: 404 },
      );
    }

    shifts.splice(index, 1);
    saveShifts(shifts);

    return NextResponse.json({ success: true });
  } catch (error) {
    return NextResponse.json(
      { error: "シフトの削除に失敗しました。" },
      { status: 500 },
    );
  }
}
