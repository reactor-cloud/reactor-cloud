import { NextResponse } from "next/server";

export async function GET(request: Request) {
  const url = new URL(request.url);
  
  return NextResponse.json({
    message: "Hello from Next.js API route!",
    timestamp: new Date().toISOString(),
    url: url.pathname,
  });
}

export async function POST(request: Request) {
  const body = await request.json().catch(() => ({}));
  
  return NextResponse.json({
    message: "POST received",
    body,
    timestamp: new Date().toISOString(),
  });
}
