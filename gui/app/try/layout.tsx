import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "トライアル実行指示" };

export default function TrialLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
