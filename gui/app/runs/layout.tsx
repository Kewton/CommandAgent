import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "実行詳細" };

export default function RunsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
