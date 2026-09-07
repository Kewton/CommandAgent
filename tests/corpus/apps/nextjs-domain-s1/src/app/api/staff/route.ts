import { NextRequest, NextResponse } from "next/server";
import { promises as fs } from "fs";
import path from "path";

const DATA_DIR = path.join(process.cwd(), "data");
const STAFF_FILE = path.join(DATA_DIR, "staff.json");

export type Role = "責任者" | "キッチン" | "ホール";

export interface Staff {
  id: string;
  name: string;
  role: Role;
}

async function readStaff(): Promise<Staff[]> {
  await fs.mkdir(DATA_DIR, { recursive: true });
  try {
    const content = await fs.readFile(STAFF_FILE, "utf-8");
    return JSON.parse(content) as Staff[];
  } catch {
    return [];
  }
}

async function writeStaff(staff: Staff[]): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
  await fs.writeFile(STAFF_FILE, JSON.stringify(staff, null, 2), "utf-8");
}

export async function GET() {
  try {
    const staff = await readStaff();
    return NextResponse.json(staff);
  } catch (err) {
    console.error("GET staff error:", err);
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const { name, role } = body;
    if (!name || !name.trim()) {
      return NextResponse.json({ error: "氏名は必須です" }, { status: 400 });
    }
    if (!["責任者", "キッチン", "ホール"].includes(role)) {
      return NextResponse.json({ error: "役割は無効です" }, { status: 400 });
    }
    const staff = await readStaff();
    const newStaff: Staff = {
      id: String(Date.now()) + Math.random().toString(36).slice(2, 8),
      name: name.trim(),
      role: role as Role,
    };
    staff.push(newStaff);
    await writeStaff(staff);
    return NextResponse.json(newStaff, { status: 201 });
  } catch (err) {
    console.error("POST staff error:", err);
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function PUT(req: NextRequest) {
  try {
    const body = await req.json();
    const { id, name, role } = body;
    if (!id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }
    if (name !== undefined && !name.trim()) {
      return NextResponse.json({ error: "氏名は必須です" }, { status: 400 });
    }
    if (role !== undefined && !["責任者", "キッチン", "ホール"].includes(role)) {
      return NextResponse.json({ error: "役割は無効です" }, { status: 400 });
    }
    const staff = await readStaff();
    const idx = staff.findIndex((s) => s.id === id);
    if (idx === -1) {
      return NextResponse.json({ error: "スタッフが見つかりません" }, { status: 404 });
    }
    staff[idx] = {
      ...staff[idx],
      name: name !== undefined ? name.trim() : staff[idx].name,
      role: role !== undefined ? (role as Role) : staff[idx].role,
    };
    await writeStaff(staff);
    return NextResponse.json(staff[idx]);
  } catch (err) {
    console.error("PUT staff error:", err);
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}

export async function DELETE(req: NextRequest) {
  try {
    const body = await req.json();
    const { id } = body;
    if (!id) {
      return NextResponse.json({ error: "idは必須です" }, { status: 400 });
    }
    const staff = await readStaff();
    const filtered = staff.filter((s) => s.id !== id);
    await writeStaff(filtered);
    return NextResponse.json({ success: true });
  } catch (err) {
    console.error("DELETE staff error:", err);
    return NextResponse.json({ error: "サーバーエラーが発生しました" }, { status: 500 });
  }
}
