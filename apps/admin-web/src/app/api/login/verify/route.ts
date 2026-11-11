import { NextRequest, NextResponse } from 'next/server';
import { setSessionCookies, backendUrl, backendHeaders } from '@/lib/session';
import { readAuthBody, AuthBodyError } from '@/lib/turnstile';

// Step 2 of admin login: the session_token from /api/login plus the
// one-time code the admin received by email. Proxies to the Rust backend's
// /auth/admin/verify-otp — identical shape to Doctor/Patient's own
// verify-otp step; Admin's mandatory authenticator (TOTP) second factor was
// removed (see docs/AUTH_SECURITY.md §2.4). Only on success here are the
// real access/refresh tokens stored in HttpOnly cookies — client-side
// JavaScript never sees them at either step, which is the whole point of
// the BFF pattern over storing a bearer token in localStorage.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try {
    body = await readAuthBody(req);
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Invalid sign-in form.' }, { status: error instanceof AuthBodyError ? error.status : 400 });
  }

  const { session_token, otp_code } = body;
  if (typeof session_token !== 'string' || typeof otp_code !== 'string' || !session_token || session_token.length > 256 || !/^[0-9]{6}$/.test(otp_code)) {
    return NextResponse.json({ success: false, error: 'session_token and otp_code are required.' }, { status: 400 });
  }

  let backendRes: Response;
  try {
    backendRes = await fetch(`${backendUrl()}/api/v1/auth/admin/verify-otp`, {
      method: 'POST',
      headers: { ...(await backendHeaders()), 'Content-Type': 'application/json' },
      body: JSON.stringify({ session_token, otp_code }),
      cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000),
    });
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502 });
  }

  const payload = await backendRes.json().catch(() => null);
  if (!backendRes.ok || !payload?.success) {
    const message = payload?.error?.message || 'Invalid or expired verification code.';
    return NextResponse.json({ success: false, error: message }, { status: backendRes.status });
  }

  const { access_token, refresh_token, user } = payload.data;
  await setSessionCookies(access_token, refresh_token);

  return NextResponse.json({ success: true, data: { user } }, { headers: { 'Cache-Control': 'private, no-store' } });
}
