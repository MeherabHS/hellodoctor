import { NextRequest, NextResponse } from 'next/server';
import { coordinatedRefresh } from '@/lib/refresh';
import { getAccessToken, getRefreshToken, setSessionCookies, clearSessionCookies, backendUrl, backendHeaders } from '@/lib/session';

// The only place the browser's requests ever reach the Rust backend from.
// Every admin-web API call (`src/lib/api.ts`) goes through this same-origin
// proxy rather than calling the backend directly, so the bearer token never
// has to live anywhere client-side JavaScript can read it — it's attached
// here, server-side, from the HttpOnly cookie. On a 401 (access token
// expired — they're short-lived, 15 minutes), this silently refreshes once
// using the refresh-token cookie and retries, so the admin doesn't get
// logged out every 15 minutes of active use.
async function forward(req: NextRequest, path: string[]): Promise<NextResponse> {
  const accessToken = await getAccessToken();
  if (!accessToken) {
    return NextResponse.json({ success: false, error: 'Not authenticated.' }, { status: 401 });
  }

  const targetUrl = `${backendUrl()}/api/v1/${path.join('/')}${req.nextUrl.search}`;
  const method = req.method;
  const hasBody = method !== 'GET' && method !== 'HEAD';
  const bodyText = hasBody ? await req.text() : undefined;

  const doFetch = async (token: string) =>
    fetch(targetUrl, {
      method,
      headers: {
        ...(await backendHeaders()),
        Authorization: `Bearer ${token}`,
        ...(hasBody ? { 'Content-Type': 'application/json' } : {}),
      },
      body: bodyText,
      cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000),
    });

  let backendRes: Response;
  try {
    backendRes = await doFetch(accessToken);
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502 });
  }

  if (backendRes.status === 401) {
    const refreshToken = await getRefreshToken();
    if (refreshToken) {
      try {
        const tokens = await coordinatedRefresh(refreshToken, async () => {
          const refreshRes = await fetch(`${backendUrl()}/api/v1/auth/refresh`, {
            method: 'POST', headers: { ...(await backendHeaders()), 'Content-Type': 'application/json' },
            body: JSON.stringify({ refresh_token: refreshToken }), cache: 'no-store',
            redirect: 'error', signal: AbortSignal.timeout(15000),
          });
          const payload = await refreshRes.json().catch(() => null);
          return refreshRes.ok && payload?.success ? payload.data : null;
        });
        if (tokens) {
          await setSessionCookies(tokens.access_token, tokens.refresh_token);
          backendRes = await doFetch(tokens.access_token);
        } else {
          await clearSessionCookies();
        }
      } catch {
        // Backend unreachable for the refresh attempt — fall through and
        // return the original 401 rather than masking it as something else.
      }
    }
  }

  const responseBody = await backendRes.text();
  return new NextResponse(responseBody, {
    status: backendRes.status,
    headers: { 'Content-Type': backendRes.headers.get('Content-Type') || 'application/json', 'Cache-Control': 'private, no-store' },
  });
}

export async function GET(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function POST(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function PATCH(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
export async function DELETE(req: NextRequest, props: { params: Promise<{ path: string[] }> }) {
  const params = await props.params;
  return forward(req, params.path);
}
