import { NextRequest } from 'next/server';
import { readAuthBody, AuthBodyError } from '@/lib/turnstile';
import { TOKEN_PATTERN, fail, forward } from '../../_public/forward';

// Public: which email an open invitation is for.
export async function POST(req: NextRequest) {
  let body: Record<string, unknown>;
  try { body = await readAuthBody(req); }
  catch (error) { return fail('Invalid request.', error instanceof AuthBodyError ? error.status : 400); }
  if (!body || typeof body !== 'object' || Array.isArray(body)) return fail('Invalid request.', 400);
  const token = body.token;
  if (typeof token !== 'string' || !TOKEN_PATTERN.test(token)) return fail('This invitation link is invalid or has expired.', 401);
  return forward('admin/invitations/lookup', { token: token.toLowerCase() });
}
