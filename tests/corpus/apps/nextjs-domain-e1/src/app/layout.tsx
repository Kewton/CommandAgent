import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "経費申請・予算承認システム",
  description: "小規模制作会社向けの経費申請・予算承認アプリ",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="ja">
      <body className="bg-gray-50 text-gray-900 antialiased">{children}</body>
    </html>
  );
}
