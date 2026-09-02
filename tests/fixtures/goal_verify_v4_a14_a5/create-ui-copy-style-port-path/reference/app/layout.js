export const metadata = { title: "Play" };

export default function RootLayout({ children }) {
  return (
    <html lang="ja">
      <body style={{ backgroundColor: "blue", margin: 0 }}>{children}</body>
    </html>
  );
}
