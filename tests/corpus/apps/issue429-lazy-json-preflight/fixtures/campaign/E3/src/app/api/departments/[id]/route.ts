import { NextRequest, NextResponse } from "next/server";
import { getDepartments, saveDepartment, deleteDepartment } from "@/lib/db";
import { Department } from "@/lib/types";

export async function PUT(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const { id } = params;
    const body = await request.json();
    const name = body.name;
    const monthlyBudget = body.monthlyBudget;

    if (!name || typeof name !== "string" || name.trim() === "") {
      return NextResponse.json({ error: "部門名は必須です" }, { status: 400 });
    }
    if (typeof monthlyBudget !== "number" || monthlyBudget <= 0) {
      return NextResponse.json({ error: "月次予算は正の数で指定してください" }, { status: 400 });
    }

    // Check existence
    const depts = await getDepartments();
    const found = depts.find((d) => d.id === id);
    if (!found) {
      return NextResponse.json({ error: "部門が見つかりません" }, { status: 404 });
    }

    const updated = await saveDepartment({ id, name: name.trim(), monthlyBudget });
    return NextResponse.json(updated);
  } catch (err) {
    return NextResponse.json({ error: "更新に失敗しました" }, { status: 500 });
  }
}

export async function DELETE(
  _request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const { id } = params;
    const depts = await getDepartments();
    const found = depts.find((d) => d.id === id);
    if (!found) {
      return NextResponse.json({ error: "部門が見つかりません" }, { status: 404 });
    }
    await deleteDepartment(id);
    return NextResponse.json({ success: true });
  } catch (err) {
    return NextResponse.json({ error: "削除に失敗しました" }, { status: 500 });
  }
}
