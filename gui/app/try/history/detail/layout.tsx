import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "トライアル実行結果詳細" };

export default function TrialDetailLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
