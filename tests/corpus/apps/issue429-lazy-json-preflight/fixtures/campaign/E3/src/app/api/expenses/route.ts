import { NextRequest, NextResponse } from "next/server";
import { getExpenses, getDepartments, saveExpense } from "@/lib/db";

export async function GET(request: NextRequest) {
  try {
    const all = await getExpenses();
    const url = new URL(request.url);
    const month = url.searchParams.get("month");
    const department = url.searchParams.get("department");
    const status = url.searchParams.get("status");
    const applicant = url.searchParams.get("applicant");

    let filtered = all;

    if (month) {
      filtered = filtered.filter((e) => e.date.startsWith(month));
    }
    if (department) {
      filtered = filtered.filter((e) => e.departmentId === department);
    }
    if (status) {
      filtered = filtered.filter((e) => e.status === status);
    }
    if (applicant) {
      filtered = filtered.filter((e) => e.applicant === applicant);
    }

    return NextResponse.json(filtered);
  } catch (err) {
    return NextResponse.json({ error: "読み込みに失敗しました" }, { status: 500 });
  }
}

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { applicant, departmentId, date, category, amount, purpose } = body;

    if (!applicant || typeof applicant !== "string" || applicant.trim() === "") {
      return NextResponse.json({ error: "申請者は必須です" }, { status: 400 });
    }
    if (!departmentId || typeof departmentId !== "string") {
      return NextResponse.json({ error: "部門は必須です" }, { status: 400 });
    }
    if (!date || typeof date !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
      return NextResponse.json({ error: "利用日はYYYY-MM-DD形式で指定してください" }, { status: 400 });
    }
    if (!category || typeof category !== "string" || category.trim() === "") {
      return NextResponse.json({ error: "カテゴリは必須です" }, { status: 400 });
    }
    if (typeof amount !== "number" || amount <= 0) {
      return NextResponse.json({ error: "金額は正の数で指定してください" }, { status: 400 });
    }
    if (!purpose || typeof purpose !== "string" || purpose.trim() === "") {
      return NextResponse.json({ error: "目的は必須です" }, { status: 400 });
    }

    // Validate department exists
    const depts = await getDepartments();
    const dept = depts.find((d) => d.id === departmentId);
    if (!dept) {
      return NextResponse.json({ error: "選択された部門が見つかりません" }, { status: 400 });
    }

    const expense = await saveExpense({
      applicant: applicant.trim(),
      departmentId,
      date,
      category: category.trim(),
      amount,
      purpose: purpose.trim(),
    });

    return NextResponse.json(expense, { status: 201 });
  } catch (err) {
    return NextResponse.json({ error: "保存に失敗しました" }, { status: 500 });
  }
}
