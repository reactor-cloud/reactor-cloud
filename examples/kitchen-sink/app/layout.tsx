import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "SSR Smoke Test",
  description: "Testing Next.js standalone mode with Reactor",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
