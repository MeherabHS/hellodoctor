import { NextRequest } from 'next/server';
import { readAuthBody, AuthBodyError } from '@/lib/turnstile';
import { fail, forward } from '../../_public/forward';

// Step 3 (public): set the new password. The authorization is consumed once;
// every existing session for the account is signed out by the backend.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try { body = await readAuthBody(req); }
  catch (error) { return fail('Invalid request.', error instanceof AuthBodyError ? error.status : 400); }
  if (!body || typeof body !== 'object' || Array.isArray(body)) return fail('Invalid request.', 400);
  const { reset_authorization, new_password } = body;
  if (typeof reset_authorization !== 'string' || !/^[A-Za-z0-9-]{16,256}$/.test(reset_authorization)) return fail('This reset session has expired. Please start again.', 401);
  if (typeof new_password !== 'string' || new_password.length < 8 || new_password.length > 128) return fail('Use a password of 8 to 128 characters.', 422);
  return forward('password-reset/complete', { reset_authorization, new_password });
}
