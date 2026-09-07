import { NextRequest, NextResponse } from "next/server";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";

export const dynamic = "force-dynamic";

const DATA_DIR = join(process.cwd(), "data");
const STAFF_FILE = join(DATA_DIR, "staff.json");

const ALLOWED_ROLES = ["責任者", "キッチン", "ホール"] as const;
type Role = (typeof ALLOWED_ROLES)[number];

interface Staff {
  id: string;
  name: string;
  role: Role;
}

function readStaff(): Staff[] {
  if (!existsSync(STAFF_FILE)) {
    return [];
  }
  const raw = readFileSync(STAFF_FILE, "utf-8");
  return JSON.parse(raw) as Staff[];
}

function writeStaff(staff: Staff[]): void {
  writeFileSync(STAFF_FILE, JSON.stringify(staff, null, 2), "utf-8");
}

// GET /api/staff/:id
export async function GET(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const staff = readStaff();
    const found = staff.find((s) => s.id === params.id);
    if (!found) {
      return NextResponse.json({ error: "スタッフが見つかりません" }, { status: 404 });
    }
    return NextResponse.json(found);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

// PUT /api/staff/:id
export async function PUT(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const body = await request.json();
    const { name, role } = body;

    if (!name || typeof name !== "string" || name.trim() === "") {
      return NextResponse.json(
        { error: "氏名は必須です" },
        { status: 400 }
      );
    }

    if (!role || !ALLOWED_ROLES.includes(role as Role)) {
      return NextResponse.json(
        { error: `役割は「${ALLOWED_ROLES.join("」「")}」のいずれかを選択してください` },
        { status: 400 }
      );
    }

    const staff = readStaff();
    const idx = staff.findIndex((s) => s.id === params.id);
    if (idx === -1) {
      return NextResponse.json({ error: "スタッフが見つかりません" }, { status: 404 });
    }

    staff[idx] = {
      ...staff[idx],
      name: name.trim(),
      role: role as Role,
    };

    writeStaff(staff);

    return NextResponse.json(staff[idx]);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

// DELETE /api/staff/:id
export async function DELETE(
  request: NextRequest,
  { params }: { params: { id: string } }
) {
  try {
    const staff = readStaff();
    const idx = staff.findIndex((s) => s.id === params.id);
    if (idx === -1) {
      return NextResponse.json({ error: "スタッフが見つかりません" }, { status: 404 });
    }

    staff.splice(idx, 1);
    writeStaff(staff);

    return NextResponse.json({ success: true });
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}
