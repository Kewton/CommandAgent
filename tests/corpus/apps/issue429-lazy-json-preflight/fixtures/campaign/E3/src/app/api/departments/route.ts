import { NextRequest, NextResponse } from "next/server";
import { getDepartments, saveDepartment, seed } from "@/lib/db";
import { Department } from "@/lib/types";

export async function GET() {
  try {
    const departments = await getDepartments();
    return NextResponse.json(departments);
  } catch (err) {
    return NextResponse.json({ error: "読み込みに失敗しました" }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const name = body.name;
    const monthlyBudget = body.monthlyBudget;

    if (!name || typeof name !== "string" || name.trim() === "") {
      return NextResponse.json({ error: "部門名は必須です" }, { status: 400 });
    }
    if (typeof monthlyBudget !== "number" || monthlyBudget <= 0) {
      return NextResponse.json({ error: "月次予算は正の数で指定してください" }, { status: 400 });
    }

    const dept = await saveDepartment({ name: name.trim(), monthlyBudget });
    return NextResponse.json(dept, { status: 201 });
  } catch (err) {
    return NextResponse.json({ error: "保存に失敗しました" }, { status: 500 });
  }
}
