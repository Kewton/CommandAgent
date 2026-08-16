import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "アセット" };

export default function AssetsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
