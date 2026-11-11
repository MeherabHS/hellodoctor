import { NextRequest, NextResponse } from 'next/server';
import { backendUrl, backendHeaders } from '@/lib/session';
import { verifyChallenge, readAuthBody, AuthBodyError } from '@/lib/turnstile';
import { webAuthAssertion } from '@/lib/web-assertion';

// Step 1 verifies the browser challenge before forwarding password credentials.
// The backend emails a short-lived OTP challenge and issues no session here.
// Step 2 requires that challenge plus the emailed OTP — Admin, Doctor and
// Patient share this same single-factor-OTP MFA shape; there is no
// authenticator-app factor.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try {
    body = await readAuthBody(req);
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Invalid sign-in form.' }, { status: error instanceof AuthBodyError ? error.status : 400 });
  }

  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return NextResponse.json({ success: false, error: 'Malformed request body.' }, { status: 400 });
  }
  const { email, password } = body;
  if (typeof email !== 'string' || typeof password !== 'string' || !email || !password || email.length > 254 || password.length > 128) {
    return NextResponse.json({ success: false, error: 'email and password are required.' }, { status: 400 });
  }

  const challenge = await verifyChallenge(body.turnstile_token, 'admin_login', {
    environment: process.env.ENVIRONMENT, origin: process.env.ADMIN_WEB_ORIGIN,
    secret: process.env.TURNSTILE_SECRET_KEY,
  });
  if (!challenge.ok) return NextResponse.json({ success: false, error: "We couldn't complete sign-in. Please try again." },
    { status: challenge.status, headers: { 'Cache-Control': 'private, no-store', ...(challenge.status === 503 ? { 'Retry-After': '5' } : {}) } });

  const upstreamBody = JSON.stringify({ email, password });
  let assertion: Record<string, string>;
  try { assertion = webAuthAssertion(upstreamBody, process.env.WEB_AUTH_ASSERTION_SECRET, process.env.ENVIRONMENT); }
  catch { return NextResponse.json({ success: false, error: "We couldn't complete sign-in. Please try again." }, { status: 503 }); }
  let backendRes: Response;
  try {
    backendRes = await fetch(`${backendUrl()}/api/v1/auth/admin/login`, {
      method: 'POST',
      headers: { ...(await backendHeaders()), ...assertion, 'Content-Type': 'application/json' },
      body: upstreamBody,
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

  const { session_token, debug_otp_code } = payload.data;
  return NextResponse.json({ success: true, data: { session_token,
    debug_otp_code: process.env.ENVIRONMENT === 'development' && process.env.DEV_AUTH_OTP_ENABLED === 'true' ? debug_otp_code : undefined } },
    { headers: { 'Cache-Control': 'private, no-store' } });
}
