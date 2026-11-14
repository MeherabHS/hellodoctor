import { NextRequest, NextResponse } from 'next/server';
import { readAuthBody, AuthBodyError, verifyChallenge } from '@/lib/turnstile';
import { NO_STORE, fail, forward } from '../../_public/forward';

// Step 1 (public): email a reset code. The backend answers identically
// whether or not the email has an account (no account discovery).
// Plain ASCII only, like the backend: no lookalike or invisible characters.
const EMAIL = /^[A-Za-z0-9._%+-]{1,64}@[A-Za-z0-9.-]{3,253}$/;
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try { body = await readAuthBody(req); }
  catch (error) { return fail('Invalid request.', error instanceof AuthBodyError ? error.status : 400); }
  if (!body || typeof body !== 'object' || Array.isArray(body)) return fail('Invalid request.', 400);
  const email = typeof body.email === 'string' ? body.email.trim() : '';
  if (!EMAIL.test(email)) return fail('Enter a valid email address.', 422);
  const challenge = await verifyChallenge(body.turnstile_token, 'admin_password_reset', {
    environment: process.env.ENVIRONMENT, origin: process.env.ADMIN_WEB_ORIGIN, secret: process.env.TURNSTILE_SECRET_KEY,
  });
  if (!challenge.ok) return fail("We couldn't verify this browser. Please try again.", challenge.status);
  const res = await forward('password-reset/request', { email });
  if (!res.ok) return res;
  const payload = await res.json();
  const dev = process.env.ENVIRONMENT === 'development' && process.env.DEV_AUTH_OTP_ENABLED === 'true';
  return NextResponse.json({ success: true, data: { message: payload.data?.message, debug_otp_code: dev ? payload.data?.debug_otp_code : undefined } }, { headers: NO_STORE });
}
