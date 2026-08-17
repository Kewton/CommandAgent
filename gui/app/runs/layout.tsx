import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "検証・運用レポート" };

export default function RunsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
