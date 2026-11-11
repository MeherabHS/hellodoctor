import { NextRequest, NextResponse } from 'next/server';
import { withAdminSession, backendUrl, backendHeaders } from '@/lib/session';

// The only place the browser's requests ever reach the Rust backend from.
// Every admin-web API call (`src/lib/api.ts`) goes through this same-origin
// proxy rather than calling the backend directly, so the bearer token never
// has to live anywhere client-side JavaScript can read it — it's attached
// here, server-side, from the HttpOnly cookie. On a 401 (access token
// expired — they're short-lived, 15 minutes), this silently refreshes once
// using the refresh-token cookie and retries, so the admin doesn't get
// logged out every 15 minutes of active use.
async function forward(req: NextRequest, path: string[]): Promise<NextResponse> {
  const targetUrl = `${backendUrl()}/api/v1/${path.join('/')}${req.nextUrl.search}`;
  const hasBody = req.method !== 'GET' && req.method !== 'HEAD';
  let body: Promise<string> | undefined;
  const backendRes = await withAdminSession(async token => fetch(targetUrl, {
    method: req.method,
    headers: { ...(await backendHeaders()), Authorization: `Bearer ${token}`,
      'X-Request-Id': req.headers.get('X-Request-Id') || crypto.randomUUID(),
      'X-Client-Type': 'ADMIN_WEB',
      ...(req.headers.get('Idempotency-Key') ? {'Idempotency-Key':req.headers.get('Idempotency-Key')!} : {}),
      ...(hasBody ? {'Content-Type':'application/json'} : {}) },
    body: hasBody ? await (body ??= req.text()) : undefined,
    cache:'no-store', redirect:'error', signal:AbortSignal.timeout(15000),
  }));
  return new NextResponse(await backendRes.text(), { status:backendRes.status,
    headers:{'X-Request-Id':backendRes.headers.get('X-Request-Id')||'',
      'Content-Type':backendRes.headers.get('Content-Type')||'application/json',
      'Cache-Control':'private, no-store',
      ...(backendRes.headers.has('Retry-After')?{'Retry-After':backendRes.headers.get('Retry-After')!}:{})} });
}

export async function GET(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function POST(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function PATCH(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function DELETE(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
