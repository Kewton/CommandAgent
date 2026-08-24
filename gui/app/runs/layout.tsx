import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "リポジトリ実行記録" };

export default function RunsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
