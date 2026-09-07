import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "カフェ週間シフト編成",
  description: "小さなカフェ向けの週間シフト編成アプリ",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ja">
      <body className="bg-slate-50 text-slate-800 antialiased">{children}</body>
    </html>
  );
}
