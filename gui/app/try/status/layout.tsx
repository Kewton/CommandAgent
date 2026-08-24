import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "トライアル実行状況" };

export default function TrialStatusLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
