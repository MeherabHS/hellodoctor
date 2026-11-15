import { NextRequest } from 'next/server';
import { readAuthBody, AuthBodyError, verifyChallenge } from '@/lib/turnstile';
import { TOKEN_PATTERN, fail, forward } from '../../_public/forward';

// Public: accept an Admin invitation (sets phone number and password). Same
// browser challenge as Admin sign-in; the backend validates everything again.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try { body = await readAuthBody(req); }
  catch (error) { return fail('Invalid request.', error instanceof AuthBodyError ? error.status : 400); }
  if (!body || typeof body !== 'object' || Array.isArray(body)) return fail('Invalid request.', 400);
  const { token, phone_number, password } = body;
  if (typeof token !== 'string' || !TOKEN_PATTERN.test(token)) return fail('This invitation link is invalid or has expired.', 401);
  if (typeof phone_number !== 'string' || !/^\+?[0-9 ()-]{9,20}$/.test(phone_number)) return fail('Enter a valid Kenyan phone number.', 422);
  if (typeof password !== 'string' || password.length < 8 || password.length > 128) return fail('Use a password of 8 to 128 characters.', 422);
  const challenge = await verifyChallenge(body.turnstile_token, 'admin_invite', {
    environment: process.env.ENVIRONMENT, origin: process.env.ADMIN_WEB_ORIGIN, secret: process.env.TURNSTILE_SECRET_KEY,
  });
  if (!challenge.ok) return fail("We couldn't verify this browser. Please try again.", challenge.status);
  return forward('admin/invitations/accept', { token: token.toLowerCase(), phone_number, password });
}
