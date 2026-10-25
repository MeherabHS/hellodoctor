import { NextRequest, NextResponse } from 'next/server';
import { exposureHeaders, ExposureError } from './lib/exposure';
import { hasPrototypePollution, hasTunnelingHeader, isJsonStateChangingRequest, isPerimeterBlockedPath } from './lib/sideDoorGuard';

function sideDoorBlocked(status: number) {
  return NextResponse.json({ success: false, error: 'Request rejected.' },
    { status, headers: { 'Cache-Control': 'no-store' } });
}

// CSP, side-door perimeter defense and COOP/CORP isolation only — Doctor
// Web's CSRF (sameOrigin()) and session/auth gating already live correctly
// in src/lib/server.ts, per-route; duplicating either here would be
// redundant at best and risks drifting out of sync with the real checks.
// This middleware exists to (1) fail closed on the same ingress-
// configuration validation every route handler already performs via
// `forwardingHeaders()`, now applied to every request including plain page
// loads that never call the backend, (2) reject request shapes that have no
// legitimate explanation — verb-tunneling/proxy-rewrite headers, SSRF/debug/
// backup-file-shaped paths, and prototype-pollution-shaped JSON bodies (see
// `lib/sideDoorGuard.ts` for why each one is safe to block unconditionally),
// and (3) set a per-request CSP nonce plus cross-origin isolation headers,
// mirroring apps/admin-web/src/middleware.ts's proven pattern exactly.
export async function middleware(req: NextRequest) {
  try {
    exposureHeaders(req.headers, { environment: process.env.ENVIRONMENT,
      origin: process.env.DOCTOR_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET,
      devOtp: process.env.DEV_AUTH_OTP_ENABLED });
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Secure deployment configuration or ingress required.' },
      { status: error instanceof ExposureError ? error.status : 503, headers: { 'Cache-Control': 'no-store' } });
  }

  if (hasTunnelingHeader(req.headers)) return sideDoorBlocked(400);
  if (isPerimeterBlockedPath(req.nextUrl.pathname)) return sideDoorBlocked(403);
  if (isJsonStateChangingRequest(req.method, req.headers.get('content-type'))) {
    try {
      const body = await req.clone().text();
      if (body && hasPrototypePollution(JSON.parse(body))) return sideDoorBlocked(403);
    } catch {
      // Malformed JSON is this route's own job to reject (Zod/the backend) —
      // this guard only ever blocks a *well-formed* pollution-shaped payload.
    }
  }

  // Next's bootstrap/Flight scripts require a per-request nonce. A bare
  // script-src 'self' prevents hydration (the app becomes a native GET form).
  const nonce = Buffer.from(crypto.randomUUID()).toString('base64');
  const dev = process.env.NODE_ENV !== 'production';
  const csp = ["default-src 'self'", `script-src 'self' 'nonce-${nonce}' https://challenges.cloudflare.com${dev ? " 'unsafe-eval'" : ''}`,
    "style-src 'self' 'unsafe-inline'", "img-src 'self' data: blob:", "font-src 'self'",
    `connect-src 'self'${dev ? ' ws://127.0.0.1:3001' : ''}`, "object-src 'none'",
    "base-uri 'none'", "frame-src https://challenges.cloudflare.com", "frame-ancestors 'none'", "form-action 'self'"].join('; ');
  const requestHeaders = new Headers(req.headers);
  requestHeaders.set('x-nonce', nonce);
  requestHeaders.set('Content-Security-Policy', csp);
  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set('Content-Security-Policy', csp);
  // Side-channel isolation: COOP prevents a cross-origin popup/opener from
  // sharing this page's window object (Spectre-class memory leaks, opener
  // hijacking); CORP stops a third-party page from side-loading this
  // origin's responses as a sub-resource. Neither is covered by the CSP
  // `frame-ancestors`/`form-action` directives already set above.
  response.headers.set('Cross-Origin-Opener-Policy', 'same-origin');
  response.headers.set('Cross-Origin-Resource-Policy', 'same-origin');
  return response;
}

export const config = {
  // Skip static assets and Next internals; apply to every page and API route otherwise.
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
