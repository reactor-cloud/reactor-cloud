import { headers } from "next/headers";

export const dynamic = "force-dynamic";

export default function Home() {
  const headersList = headers();
  const userAgent = headersList.get("user-agent") || "unknown";
  const timestamp = new Date().toISOString();

  return (
    <main style={{ fontFamily: "system-ui", padding: "2rem" }}>
      <h1>SSR Smoke Test</h1>
      <p>This page is server-side rendered on every request.</p>
      
      <section style={{ marginTop: "2rem" }}>
        <h2>Dynamic Data</h2>
        <ul>
          <li><strong>Timestamp:</strong> {timestamp}</li>
          <li><strong>User-Agent:</strong> {userAgent.substring(0, 80)}...</li>
        </ul>
      </section>

      <section style={{ marginTop: "2rem" }}>
        <h2>Test Links</h2>
        <ul>
          <li>
            <a href="/api/hello" style={{ color: "blue" }}>
              /api/hello - API Route
            </a>
          </li>
        </ul>
      </section>

      <style>{`
        h1 { color: #333; }
        h2 { color: #666; margin-top: 1.5rem; }
        ul { list-style: none; padding: 0; }
        li { margin: 0.5rem 0; }
      `}</style>
    </main>
  );
}
