import { NextRequest, NextResponse } from "next/server";
import { getStaff, addStaff, generateId } from "@/lib/storage";
import { validateStaff } from "@/lib/validation";
import type { Staff } from "@/lib/types";

// GET: スタッフ一覧を取得
export async function GET() {
  try {
    const staff = await getStaff();
    return NextResponse.json({ staff });
  } catch (err) {
    console.error("スタッフ取得エラー:", err);
    return NextResponse.json(
      { error: "スタッフの取得に失敗しました。" },
      { status: 500 }
    );
  }
}

// POST: 新規スタッフ追加
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const result = validateStaff(body);
    if (!result.valid) {
      return NextResponse.json(
        { error: result.errors[0], errors: result.errors },
        { status: 400 }
      );
    }

    const staff: Staff = {
      id: generateId("staff"),
      name: String(body.name).trim(),
      role: body.role,
    };

    const updated = await addStaff(staff);
    return NextResponse.json({ staff: updated, created: staff }, { status: 201 });
  } catch (err) {
    console.error("スタッフ追加エラー:", err);
    return NextResponse.json(
      { error: "スタッフの追加に失敗しました。" },
      { status: 500 }
    );
  }
}
