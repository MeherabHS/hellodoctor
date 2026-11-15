import { NextRequest } from 'next/server';
import { readAuthBody, AuthBodyError } from '@/lib/turnstile';
import { fail, forward } from '../../_public/forward';

// Step 2 (public): exchange the emailed 6-digit code for a short-lived,
// single-use reset authorization.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try { body = await readAuthBody(req); }
  catch (error) { return fail('Invalid request.', error instanceof AuthBodyError ? error.status : 400); }
  if (!body || typeof body !== 'object' || Array.isArray(body)) return fail('Invalid request.', 400);
  const email = typeof body.email === 'string' ? body.email.trim() : '';
  const code = typeof body.otp_code === 'string' ? body.otp_code.trim() : '';
  if (!email || email.length > 254 || !/^[0-9]{6}$/.test(code)) return fail('Enter the 6-digit code from the email.', 422);
  return forward('password-reset/verify-otp', { email, otp_code: code });
}
