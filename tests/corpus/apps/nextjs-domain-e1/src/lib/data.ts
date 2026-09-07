import { promises as fs } from "fs";
import path from "path";

// ─── Types ────────────────────────────────────────────────────────────────

export interface Department {
  id: string;
  name: string;
  monthlyBudget: number;
}

export type ExpenseStatus = "申請中" | "承認" | "却下";

export interface Expense {
  id: string;
  applicant: string;
  departmentId: string;
  date: string; // YYYY-MM-DD
  category: string;
  amount: number;
  purpose: string;
  status: ExpenseStatus;
  rejectionReason?: string;
  approvedAt?: string;
}

export interface BudgetExceededResult {
  ok: false;
  error: "BUDGET_EXCEEDED";
  remaining: number;
  overage: number;
}

export interface ApprovalResult {
  ok: true;
  expense: Expense;
}

export type ApprovalOutcome = ApprovalResult | BudgetExceededResult;

// ─── File paths ───────────────────────────────────────────────────────────

const DATA_DIR = path.join(process.cwd(), "data");
const DEPARTMENTS_FILE = path.join(DATA_DIR, "departments.json");
const EXPENSES_FILE = path.join(DATA_DIR, "expenses.json");

// ─── Utilities ────────────────────────────────────────────────────────────

function generateId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 10);
}

async function ensureDataDir(): Promise<void> {
  await fs.mkdir(DATA_DIR, { recursive: true });
}

async function readFile<T>(filePath: string, fallback: T): Promise<T> {
  try {
    const content = await fs.readFile(filePath, "utf-8");
    return JSON.parse(content) as T;
  } catch {
    return fallback;
  }
}

async function writeFile<T>(filePath: string, data: T): Promise<void> {
  await ensureDataDir();
  await fs.writeFile(filePath, JSON.stringify(data, null, 2), "utf-8");
}

// ─── Seeding ──────────────────────────────────────────────────────────────

function currentMonthDate(day: number): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = now.getMonth() + 1;
  const d = Math.min(day, new Date(y, m, 0).getDate());
  return `${y}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
}

function buildSeedDepartments(): Department[] {
  return [
    { id: "dept-001", name: "制作部門", monthlyBudget: 500000 },
    { id: "dept-002", name: "企画部門", monthlyBudget: 300000 },
    { id: "dept-003", name: "運用部門", monthlyBudget: 200000 },
  ];
}

function buildSeedExpenses(): Expense[] {
  const today = new Date();
  const y = today.getFullYear();
  const m = today.getMonth() + 1;
  const day = (d: number) =>
    `${y}-${String(m).padStart(2, "0")}-${String(Math.min(d, new Date(y, m, 0).getDate())).padStart(2, "0")}`;

  return [
    {
      id: "exp-001",
      applicant: "田中太郎",
      departmentId: "dept-001",
      date: day(2),
      category: "交通費",
      amount: 5200,
      purpose: "クライアント先への移動",
      status: "承認",
      approvedAt: day(3),
    },
    {
      id: "exp-002",
      applicant: "田中太郎",
      departmentId: "dept-001",
      date: day(4),
      category: "飲食費",
      amount: 8500,
      purpose: "チーム会議での食事",
      status: "承認",
      approvedAt: day(5),
    },
    {
      id: "exp-003",
      applicant: "佐藤花子",
      departmentId: "dept-001",
      date: day(6),
      category: "外注費",
      amount: 120000,
      purpose: "デザイン外注",
      status: "申請中",
    },
    {
      id: "exp-004",
      applicant: "鈴木一郎",
      departmentId: "dept-002",
      date: day(3),
      category: "資料費",
      amount: 15000,
      purpose: "書籍購入",
      status: "承認",
      approvedAt: day(4),
    },
    {
      id: "exp-005",
      applicant: "鈴木一郎",
      departmentId: "dept-002",
      date: day(5),
      category: "交通費",
      amount: 3200,
      purpose: "交通費",
      status: "却下",
      rejectionReason: "領収書が見つからない",
    },
    {
      id: "exp-006",
      applicant: "山本三郎",
      departmentId: "dept-003",
      date: day(1),
      category: "雑費",
      amount: 4500,
      purpose: "備品購入",
      status: "承認",
      approvedAt: day(1),
    },
    {
      id: "exp-007",
      applicant: "山本三郎",
      departmentId: "dept-003",
      date: day(7),
      category: "飲食費",
      amount: 6800,
      purpose: "仕事中の昼食",
      status: "申請中",
    },
    {
      id: "exp-008",
      applicant: "田中太郎",
      departmentId: "dept-001",
      date: day(8),
      category: "交通費",
      amount: 2800,
      purpose: "社内移動",
      status: "申請中",
    },
    {
      id: "exp-009",
      applicant: "佐藤花子",
      departmentId: "dept-001",
      date: day(9),
      category: "外注費",
      amount: 80000,
      purpose: "動画編集外注",
      status: "却下",
      rejectionReason: "予算超過の懸念があるため却下",
    },
    {
      id: "exp-010",
      applicant: "鈴木一郎",
      departmentId: "dept-002",
      date: day(10),
      category: "資料費",
      amount: 22000,
      purpose: "調査レポート購入",
      status: "申請中",
    },
  ];
}

async function ensureSeeded(): Promise<void> {
  await ensureDataDir();
  const departments = await readFile<Department[]>(DEPARTMENTS_FILE, [] as Department[]);
  if (departments.length === 0) {
    await writeFile<Department[]>(DEPARTMENTS_FILE, buildSeedDepartments());
  }
  const expenses = await readFile<Expense[]>(EXPENSES_FILE, [] as Expense[]);
  if (expenses.length === 0) {
    await writeFile<Expense[]>(EXPENSES_FILE, buildSeedExpenses());
  }
}

// ─── Department CRUD ──────────────────────────────────────────────────────

export async function getDepartments(): Promise<Department[]> {
  await ensureSeeded();
  return readFile<Department[]>(DEPARTMENTS_FILE, []);
}

export async function getDepartmentById(id: string): Promise<Department | undefined> {
  const departments = await getDepartments();
  return departments.find((d) => d.id === id);
}

export async function createDepartment(
  data: Pick<Department, "name" | "monthlyBudget">,
): Promise<Department> {
  await ensureSeeded();
  const departments = await getDepartments();
  const dept: Department = { id: generateId(), ...data };
  departments.push(dept);
  await writeFile(DEPARTMENTS_FILE, departments);
  return dept;
}

export async function updateDepartment(
  id: string,
  updates: Partial<Pick<Department, "name" | "monthlyBudget">>,
): Promise<Department> {
  const departments = await getDepartments();
  const idx = departments.findIndex((d) => d.id === id);
  if (idx === -1) throw new Error("部門が見つかりません");
  departments[idx] = { ...departments[idx], ...updates };
  await writeFile(DEPARTMENTS_FILE, departments);
  return departments[idx];
}

export async function deleteDepartment(id: string): Promise<void> {
  const departments = await getDepartments();
  const filtered = departments.filter((d) => d.id !== id);
  await writeFile(DEPARTMENTS_FILE, filtered);
}

// ─── Expense CRUD ─────────────────────────────────────────────────────────

export async function getExpenses(): Promise<Expense[]> {
  await ensureSeeded();
  return readFile<Expense[]>(EXPENSES_FILE, []);
}

export async function getExpense(id: string): Promise<Expense | undefined> {
  const expenses = await getExpenses();
  return expenses.find((e) => e.id === id);
}

export async function createExpense(
  data: Omit<Expense, "id" | "status" | "approvedAt">,
): Promise<Expense> {
  await ensureSeeded();
  const expenses = await getExpenses();
  const expense: Expense = { id: generateId(), ...data, status: "申請中" };
  expenses.push(expense);
  await writeFile(EXPENSES_FILE, expenses);
  return expense;
}

export async function updateExpense(
  id: string,
  updates: Partial<Omit<Expense, "id" | "status" | "approvedAt">>,
): Promise<Expense> {
  const expenses = await getExpenses();
  const idx = expenses.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("経費申請が見つかりません");
  if (expenses[idx].status !== "申請中") {
    throw new Error("申請中の経費のみ編集可能です");
  }
  expenses[idx] = { ...expenses[idx], ...updates, status: "申請中" };
  await writeFile(EXPENSES_FILE, expenses);
  return expenses[idx];
}

export async function deleteExpense(id: string): Promise<void> {
  const expenses = await getExpenses();
  const idx = expenses.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("経費申請が見つかりません");
  if (expenses[idx].status !== "申請中") {
    throw new Error("申請中の経費のみ削除可能です");
  }
  expenses.splice(idx, 1);
  await writeFile(EXPENSES_FILE, expenses);
}

// ─── Budget enforcement ───────────────────────────────────────────────────

export function getApprovedTotalForDepartment(
  expenses: Expense[],
  departmentId: string,
): number {
  return expenses
    .filter((e) => e.departmentId === departmentId && e.status === "承認")
    .reduce((sum, e) => sum + e.amount, 0);
}

export async function approveExpense(id: string): Promise<ApprovalOutcome> {
  await ensureSeeded();
  const expenses = await getExpenses();
  const idx = expenses.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("経費申請が見つかりません");
  if (expenses[idx].status !== "申請中") {
    throw new Error("申請中の経費のみ承認可能です");
  }

  const departmentId = expenses[idx].departmentId;
  const dept = await getDepartmentById(departmentId);
  if (!dept) throw new Error("所属部門が見つかりません");

  const approvedTotal = getApprovedTotalForDepartment(expenses, departmentId);
  const remaining = dept.monthlyBudget - approvedTotal;

  if (approvedTotal + expenses[idx].amount > dept.monthlyBudget) {
    return {
      ok: false,
      error: "BUDGET_EXCEEDED",
      remaining,
      overage: approvedTotal + expenses[idx].amount - dept.monthlyBudget,
    };
  }

  expenses[idx] = {
    ...expenses[idx],
    status: "承認",
    approvedAt: new Date().toISOString(),
  };
  await writeFile(EXPENSES_FILE, expenses);
  return { ok: true, expense: expenses[idx] };
}

export async function rejectExpense(
  id: string,
  reason: string,
): Promise<Expense> {
  await ensureSeeded();
  const expenses = await getExpenses();
  const idx = expenses.findIndex((e) => e.id === id);
  if (idx === -1) throw new Error("経費申請が見つかりません");
  if (expenses[idx].status !== "申請中") {
    throw new Error("申請中の経費のみ却下可能です");
  }
  expenses[idx] = { ...expenses[idx], status: "却下", rejectionReason: reason };
  await writeFile(EXPENSES_FILE, expenses);
  return expenses[idx];
}

// ─── Filtering ────────────────────────────────────────────────────────────

export interface ExpenseFilter {
  month?: string; // YYYY-MM
  departmentId?: string;
  status?: ExpenseStatus;
  applicant?: string;
}

export async function getFilteredExpenses(filter: ExpenseFilter): Promise<Expense[]> {
  const expenses = await getExpenses();
  return expenses.filter((e) => {
    if (filter.month && !e.date.startsWith(filter.month)) return false;
    if (filter.departmentId && e.departmentId !== filter.departmentId) return false;
    if (filter.status && e.status !== filter.status) return false;
    if (filter.applicant && e.applicant !== filter.applicant) return false;
    return true;
  });
}

// ─── Dashboard stats ──────────────────────────────────────────────────────

export interface DashboardStats {
  byStatus: Record<string, number>;
  byDepartment: Array<{
    id: string;
    name: string;
    budget: number;
    approvedTotal: number;
    remaining: number;
  }>;
  byCategory: Array<{ category: string; total: number }>;
}

export async function getDashboardStats(): Promise<DashboardStats> {
  const [expenses, departments] = await Promise.all([getExpenses(), getDepartments()]);

  const byStatus: Record<string, number> = { 申請中: 0, 承認: 0, 却下: 0 };
  for (const e of expenses) {
    byStatus[e.status] = (byStatus[e.status] || 0) + 1;
  }

  const byDepartment = departments.map((dept) => {
    const approvedTotal = getApprovedTotalForDepartment(expenses, dept.id);
    return {
      id: dept.id,
      name: dept.name,
      budget: dept.monthlyBudget,
      approvedTotal,
      remaining: dept.monthlyBudget - approvedTotal,
    };
  });

  const categoryMap = new Map<string, number>();
  for (const e of expenses) {
    if (e.status === "承認") {
      categoryMap.set(e.category, (categoryMap.get(e.category) || 0) + e.amount);
    }
  }
  const byCategory = Array.from(categoryMap.entries())
    .map(([category, total]) => ({ category, total }))
    .sort((a, b) => b.total - a.total);

  return { byStatus, byDepartment, byCategory };
}

// ─── Custom errors ────────────────────────────────────────────────────────

export class BudgetExceededError extends Error {
  remaining: number;
  overage: number;
  constructor(remaining: number, overage: number) {
    super("予算超過");
    this.name = "BudgetExceededError";
    this.remaining = remaining;
    this.overage = overage;
  }
}

export class ExpenseNotFoundError extends Error {
  constructor() {
    super("経費申請が見つかりません");
    this.name = "ExpenseNotFoundError";
  }
}

export class DepartmentNotFoundError extends Error {
  constructor() {
    super("部門が見つかりません");
    this.name = "DepartmentNotFoundError";
  }
}

export class InvalidOperationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "InvalidOperationError";
  }
}
