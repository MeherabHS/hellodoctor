import { NextResponse } from 'next/server';
import { backendUrl, backendHeaders } from '@/lib/session';

// Shared by the public (pre-session) routes: invitations and password reset.
// only in request bodies (never a URL the servers log) and is checked for
// shape here before anything is forwarded.
export const TOKEN_PATTERN = /^[0-9a-f]{64}$/i;
export const NO_STORE = { 'Cache-Control': 'private, no-store', 'Referrer-Policy': 'no-referrer' };

export function fail(error: string, status: number) {
  return NextResponse.json({ success: false, error }, { status, headers: NO_STORE });
}

export async function forward(path: string, payload: Record<string, string>) {
  let res: Response;
  try {
    res = await fetch(`${backendUrl()}/api/v1/auth/${path}`, {
      method: 'POST',
      headers: { ...(await backendHeaders()), 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
      cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000),
    });
  } catch {
    return fail('The service is unavailable. Please try again.', 502);
  }
  const body = await res.json().catch(() => null);
  if (!res.ok || !body?.success) {
    // Only the backend's user-facing message; never internal detail.
    const message = typeof body?.error?.message === 'string' ? body.error.message : 'This request could not be completed.';
    return fail(message, res.status >= 400 ? res.status : 502);
  }
  return NextResponse.json({ success: true, data: body.data }, { headers: NO_STORE });
}
