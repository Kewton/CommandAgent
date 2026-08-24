import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "トライアル実行履歴" };

export default function TrialHistoryLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
