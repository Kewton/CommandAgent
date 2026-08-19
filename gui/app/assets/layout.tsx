import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "拡張" };

export default function AssetsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
