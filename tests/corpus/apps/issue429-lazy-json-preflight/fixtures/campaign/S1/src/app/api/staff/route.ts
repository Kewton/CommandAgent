import { NextRequest, NextResponse } from "next/server";
import { readFileSync, writeFileSync, existsSync } from "fs";
import { join } from "path";
import { randomUUID } from "crypto";

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

// GET /api/staff - return all staff
export async function GET() {
  try {
    const staff = readStaff();
    return NextResponse.json(staff);
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

// POST /api/staff - create a new staff member
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const { name, role } = body;

    // Validate name
    if (!name || typeof name !== "string" || name.trim() === "") {
      return NextResponse.json(
        { error: "氏名は必須です" },
        { status: 400 }
      );
    }

    // Validate role
    if (!role || !ALLOWED_ROLES.includes(role as Role)) {
      return NextResponse.json(
        { error: `役割は「${ALLOWED_ROLES.join("」「")}」のいずれかを選択してください` },
        { status: 400 }
      );
    }

    const staff = readStaff();
    const newStaff: Staff = {
      id: randomUUID(),
      name: name.trim(),
      role: role as Role,
    };

    staff.push(newStaff);
    writeStaff(staff);

    return NextResponse.json(newStaff, { status: 201 });
  } catch (error) {
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}
