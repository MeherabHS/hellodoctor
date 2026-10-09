import { NextResponse } from 'next/server';
import { clearSessionCookies, getRefreshToken, backendUrl, backendHeaders } from '@/lib/session';

export async function POST() {
  const refresh = await getRefreshToken();
  await clearSessionCookies();
  if (refresh) {
    try {
      const response = await fetch(`${backendUrl()}/api/v1/auth/logout`, {
        method: 'POST', headers: { ...(await backendHeaders()), 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refresh }), cache: 'no-store',
        redirect: 'error', signal: AbortSignal.timeout(15000),
      });
      if (!response.ok) throw new Error('Revocation unconfirmed');
    } catch {
      return NextResponse.json({ success: false, error: 'Signed out locally. Server revocation could not be confirmed.' },
        { status: 503, headers: { 'Cache-Control': 'private, no-store' } });
    }
  }
  return NextResponse.json({ success: true }, { headers: { 'Cache-Control': 'private, no-store' } });
}
