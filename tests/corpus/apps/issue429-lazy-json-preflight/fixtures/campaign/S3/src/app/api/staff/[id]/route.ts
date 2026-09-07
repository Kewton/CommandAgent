import { NextRequest, NextResponse } from "next/server";
import { updateStaff, deleteStaff, getStaff } from "@/lib/storage";
import { validateStaff } from "@/lib/validation";

// PATCH: スタッフ編集
export async function PATCH(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const body = await request.json();
    const result = validateStaff({ ...body, name: body.name, role: body.role });
    if (!result.valid) {
      return NextResponse.json(
        { error: result.errors[0], errors: result.errors },
        { status: 400 }
      );
    }

    try {
      const updated = await updateStaff(params.id, {
        name: String(body.name).trim(),
        role: body.role,
      });
      return NextResponse.json({ staff: updated });
    } catch (err) {
      if (err instanceof Error && err.message === "スタッフが見つかりません。") {
        return NextResponse.json({ error: "スタッフが見つかりません。" }, { status: 404 });
      }
      throw err;
    }
  } catch (err) {
    console.error("スタッフ編集エラー:", err);
    return NextResponse.json(
      { error: "スタッフの編集に失敗しました。" },
      { status: 500 }
    );
  }
}

// DELETE: スタッフ削除
export async function DELETE(
  _request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    try {
      const updated = await deleteStaff(params.id);
      return NextResponse.json({ staff: updated, deleted: params.id });
    } catch (err) {
      if (err instanceof Error && err.message === "スタッフが見つかりません。") {
        return NextResponse.json({ error: "スタッフが見つかりません。" }, { status: 404 });
      }
      throw err;
    }
  } catch (err) {
    console.error("スタッフ削除エラー:", err);
    return NextResponse.json(
      { error: "スタッフの削除に失敗しました。" },
      { status: 500 }
    );
  }
}
