import { NextRequest, NextResponse } from "next/server";
import { updateShift, deleteShift, getShifts } from "@/lib/storage";
import { validateShift } from "@/lib/validation";

// PATCH: シフト編集
export async function PATCH(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const body = await request.json();
    const existingShifts = await getShifts();

    const result = validateShift(
      {
        staffId: body.staffId,
        date: body.date,
        start: body.start,
        end: body.end,
        breakMinutes: body.breakMinutes,
      },
      existingShifts,
      params.id
    );

    if (!result.valid) {
      return NextResponse.json(
        { error: result.errors[0], errors: result.errors },
        { status: 400 }
      );
    }

    try {
      const updated = await updateShift(params.id, {
        staffId: String(body.staffId),
        date: String(body.date),
        start: String(body.start),
        end: String(body.end),
        breakMinutes: Number(body.breakMinutes) || 0,
      });
      return NextResponse.json({ shifts: updated });
    } catch (err) {
      if (err instanceof Error && err.message === "シフトが見つかりません。") {
        return NextResponse.json({ error: "シフトが見つかりません。" }, { status: 404 });
      }
      throw err;
    }
  } catch (err) {
    console.error("シフト編集エラー:", err);
    return NextResponse.json(
      { error: "シフトの編集に失敗しました。" },
      { status: 500 }
    );
  }
}

// DELETE: シフト削除
export async function DELETE(
  _request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    try {
      const updated = await deleteShift(params.id);
      return NextResponse.json({ shifts: updated, deleted: params.id });
    } catch (err) {
      if (err instanceof Error && err.message === "シフトが見つかりません。") {
        return NextResponse.json({ error: "シフトが見つかりません。" }, { status: 404 });
      }
      throw err;
    }
  } catch (err) {
    console.error("シフト削除エラー:", err);
    return NextResponse.json(
      { error: "シフトの削除に失敗しました。" },
      { status: 500 }
    );
  }
}
