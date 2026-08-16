import type { Metadata } from "next";
import type { ReactNode } from "react";

export const metadata: Metadata = { title: "計測" };

export default function MeasurementsLayout({ children }: Readonly<{ children: ReactNode }>) {
  return children;
}
