import { NextRequest, NextResponse } from 'next/server';
import { setSessionCookies, backendUrl, backendHeaders } from '@/lib/session';
import { readAuthBody, AuthBodyError } from '@/lib/turnstile';
import { validateAdminVerifyBody } from '@/lib/adminVerify';

// Step 2 of admin login: the session_token from /api/login plus the
// one-time verification code the admin received by email. Proxies to the
// Rust backend's /auth/admin/verify-otp. Admin, Doctor and Patient share
// this same single-factor-OTP MFA shape; there is no authenticator-app
// factor, and the backend's AdminVerifyOtpRequest has no such field. Only
// on success here are the real access/refresh tokens stored in HttpOnly
// cookies — client-side JavaScript never sees them at either step, which
// is the whole point of the BFF pattern over storing a bearer token in
// localStorage.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try {
    body = await readAuthBody(req);
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Invalid sign-in form.' }, { status: error instanceof AuthBodyError ? error.status : 400 });
  }

  const validated = validateAdminVerifyBody(body);
  if ('error' in validated) {
    return NextResponse.json({ success: false, error: validated.error }, { status: validated.status });
  }
  const { session_token, otp_code } = validated;

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
