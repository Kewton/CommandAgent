export interface Department {
  id: string;
  name: string;
  monthlyBudget: number;
  updatedAt: string;
}

export type ExpenseStatus = "pending" | "approved" | "rejected";

export interface Expense {
  id: string;
  applicant: string;
  departmentId: string;
  date: string;
  category: string;
  amount: number;
  purpose: string;
  status: ExpenseStatus;
  rejectionReason?: string;
  createdAt: string;
  updatedAt: string;
}
