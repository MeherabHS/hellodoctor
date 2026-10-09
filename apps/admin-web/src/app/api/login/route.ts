import { NextRequest, NextResponse } from 'next/server';
import { setSessionCookies, backendUrl, backendHeaders } from '@/lib/session';

// Proxies the admin login step to the Rust backend's own
// /auth/admin/login (email + password + TOTP, all in one call — the
// backend's existing MFA design, not reinvented here). On success, the
// access/refresh tokens are stored in HttpOnly cookies server-side and
// never included in this response body — client-side JavaScript never
// sees them, which is the whole point of the BFF pattern over
// storing a bearer token in localStorage.
export async function POST(req: NextRequest) {
  let body: { email?: string; password?: string; totp_code?: string };
  try {
    body = await req.json();
  } catch {
    return NextResponse.json({ success: false, error: 'Malformed request body.' }, { status: 400 });
  }

  const { email, password, totp_code } = body;
  if (!email || !password || !totp_code) {
    return NextResponse.json({ success: false, error: 'email, password, and totp_code are required.' }, { status: 400 });
  }

  let backendRes: Response;
  try {
    backendRes = await fetch(`${backendUrl()}/api/v1/auth/admin/login`, {
      method: 'POST',
      headers: { ...(await backendHeaders()), 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password, totp_code }),
      cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000),
    });
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502 });
  }

  const payload = await backendRes.json().catch(() => null);
  if (!backendRes.ok || !payload?.success) {
    const message = payload?.error?.message || 'Invalid credentials.';
    return NextResponse.json({ success: false, error: message }, { status: backendRes.status });
  }

  const { access_token, refresh_token, user } = payload.data;
  await setSessionCookies(access_token, refresh_token);

  return NextResponse.json({ success: true, data: { user } });
}
