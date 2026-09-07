import { Department, Expense } from "./types";
import { promises as fs } from "fs";
import { join } from "path";

const DATA_DIR = join(process.cwd(), "data");
const DEPARTMENTS_FILE = join(DATA_DIR, "departments.json");
const EXPENSES_FILE = join(DATA_DIR, "expenses.json");

async function ensureDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

async function readJSON<T>(filePath: string): Promise<T[]> {
  try {
    const raw = await fs.readFile(filePath, "utf-8");
    if (!raw || raw.trim() === "") return [];
    return JSON.parse(raw) as T[];
  } catch {
    return [];
  }
}

async function writeJSON<T>(filePath: string, data: T[]): Promise<void> {
  await ensureDir();
  await fs.writeFile(filePath, JSON.stringify(data, null, 2), "utf-8");
}

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 10);
}

function now(): string {
  return new Date().toISOString();
}

// ---------- Seed ----------

export async function seed(): Promise<void> {
  await ensureDir();
  let departments = await readJSON<Department>(DEPARTMENTS_FILE);
  let expenses = await readJSON<Expense>(EXPENSES_FILE);

  if (departments.length === 0) {
    const d1: Department = {
      id: "dept-001",
      name: "映像制作部",
      monthlyBudget: 500000,
      updatedAt: now(),
    };
    const d2: Department = {
      id: "dept-002",
      name: "グラフィック部",
      monthlyBudget: 300000,
      updatedAt: now(),
    };
    departments = [d1, d2];
    await writeJSON(DEPARTMENTS_FILE, departments);
  }

  if (expenses.length === 0) {
    const t = now();
    const samples: Expense[] = [
      {
        id: "exp-001",
        applicant: "田中 太郎",
        departmentId: "dept-001",
        date: "2025-01-10",
        category: "撮影機材",
        amount: 150000,
        purpose: "新規CM撮影における機材レンタル費用",
        status: "approved",
        createdAt: "2025-01-05T09:00:00.000Z",
        updatedAt: "2025-01-06T14:00:00.000Z",
      },
      {
        id: "exp-002",
        applicant: "佐藤 花子",
        departmentId: "dept-001",
        date: "2025-01-15",
        category: "編集ソフト",
        amount: 80000,
        purpose: "Premiere Pro年間サブスクリプション更新",
        status: "pending",
        createdAt: "2025-01-12T10:00:00.000Z",
        updatedAt: "2025-01-12T10:00:00.000Z",
      },
      {
        id: "exp-003",
        applicant: "鈴木 一郎",
        departmentId: "dept-002",
        date: "2025-01-08",
        category: "印刷費",
        amount: 45000,
        purpose: "展示会用パンフレット印刷",
        status: "rejected",
        rejectionReason: "予算超過のため来月へ延期してください",
        createdAt: "2025-01-03T08:30:00.000Z",
        updatedAt: "2025-01-04T11:00:00.000Z",
      },
      {
        id: "exp-004",
        applicant: "佐藤 花子",
        departmentId: "dept-002",
        date: "2025-01-20",
        category: "素材購入",
        amount: 30000,
        purpose: "ストックフォト・動画素材の購入",
        status: "pending",
        createdAt: "2025-01-18T09:00:00.000Z",
        updatedAt: "2025-01-18T09:00:00.000Z",
      },
    ];
    expenses = samples;
    await writeJSON(EXPENSES_FILE, expenses);
  }
}

// ---------- Departments ----------

export async function getDepartments(): Promise<Department[]> {
  const depts = await readJSON<Department>(DEPARTMENTS_FILE);
  if (depts.length === 0) {
    await seed();
    return readJSON<Department>(DEPARTMENTS_FILE);
  }
  return depts;
}

export async function saveDepartment(dept: Omit<Department, "id" | "updatedAt"> & { id?: string }): Promise<Department> {
  const depts = await readJSON<Department>(DEPARTMENTS_FILE);
  const ts = now();
  if (dept.id) {
    const idx = depts.findIndex((d) => d.id === dept.id);
    if (idx === -1) throw new Error("NOT_FOUND");
    depts[idx] = { ...depts[idx], name: dept.name, monthlyBudget: dept.monthlyBudget, updatedAt: ts };
  } else {
    const newDept: Department = {
      id: generateId(),
      name: dept.name,
      monthlyBudget: dept.monthlyBudget,
      updatedAt: ts,
    };
    depts.push(newDept);
  }
  await writeJSON(DEPARTMENTS_FILE, depts);
  return depts[depts.length - 1];
}

export async function deleteDepartment(id: string): Promise<void> {
  const depts = await readJSON<Department>(DEPARTMENTS_FILE);
  const filtered = depts.filter((d) => d.id !== id);
  await writeJSON(DEPARTMENTS_FILE, filtered);
}

// ---------- Expenses ----------

export async function getExpenses(): Promise<Expense[]> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  if (exps.length === 0) {
    await seed();
    return readJSON<Expense>(EXPENSES_FILE);
  }
  return exps;
}

export async function getExpense(id: string): Promise<Expense | undefined> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  return exps.find((e) => e.id === id);
}

export async function saveExpense(
  expense: Omit<Expense, "id" | "status" | "createdAt" | "updatedAt"> & { id?: string; status?: string }
): Promise<Expense> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  const ts = now();

  if (expense.id) {
    const idx = exps.findIndex((e) => e.id === expense.id);
    if (idx === -1) throw new Error("NOT_FOUND");
    const existing = exps[idx];
    if (existing.status !== "pending") throw new Error("IMMUTABLE");
    const updated: Expense = {
      ...existing,
      applicant: expense.applicant,
      departmentId: expense.departmentId,
      date: expense.date,
      category: expense.category,
      amount: expense.amount,
      purpose: expense.purpose,
      updatedAt: ts,
    };
    exps[idx] = updated;
    await writeJSON(EXPENSES_FILE, exps);
    return updated;
  }

  const newExpense: Expense = {
    id: generateId(),
    applicant: expense.applicant,
    departmentId: expense.departmentId,
    date: expense.date,
    category: expense.category,
    amount: expense.amount,
    purpose: expense.purpose,
    status: "pending",
    createdAt: ts,
    updatedAt: ts,
  };
  exps.push(newExpense);
  await writeJSON(EXPENSES_FILE, exps);
  return newExpense;
}

export async function deleteExpense(id: string): Promise<void> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  const found = exps.find((e) => e.id === id);
  if (!found) throw new Error("NOT_FOUND");
  if (found.status !== "pending") throw new Error("IMMUTABLE");
  const filtered = exps.filter((e) => e.id !== id);
  await writeJSON(EXPENSES_FILE, filtered);
}

export async function approveExpense(id: string): Promise<Expense> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  const depts = await readJSON<Department>(DEPARTMENTS_FILE);
  const idx = exps.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("NOT_FOUND");
  const existing = exps[idx];
  if (existing.status !== "pending") throw new Error("IMMUTABLE");

  const dept = depts.find((d) => d.id === existing.departmentId);
  if (!dept) throw new Error("DEPT_NOT_FOUND");

  const approvedTotal = exps
    .filter((e) => e.departmentId === dept.id && e.status === "approved")
    .reduce((sum, e) => sum + e.amount, 0);

  const newTotal = approvedTotal + existing.amount;
  const remaining = dept.monthlyBudget - approvedTotal;

  if (newTotal > dept.monthlyBudget) {
    const excess = newTotal - dept.monthlyBudget;
    throw new Error(JSON.stringify({ BUDGET_OVERFLOW: true, remaining, excess }));
  }

  const ts = now();
  exps[idx] = { ...existing, status: "approved", updatedAt: ts };
  await writeJSON(EXPENSES_FILE, exps);
  return exps[idx];
}

export async function rejectExpense(id: string, reason: string): Promise<Expense> {
  const exps = await readJSON<Expense>(EXPENSES_FILE);
  const idx = exps.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("NOT_FOUND");
  const existing = exps[idx];
  if (existing.status !== "pending") throw new Error("IMMUTABLE");

  const ts = now();
  exps[idx] = { ...existing, status: "rejected", rejectionReason: reason, updatedAt: ts };
  await writeJSON(EXPENSES_FILE, exps);
  return exps[idx];
}
