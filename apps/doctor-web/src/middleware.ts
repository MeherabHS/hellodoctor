import { NextRequest, NextResponse } from 'next/server';
import { exposureHeaders, ExposureError } from './lib/exposure';

// CSP only — Doctor Web's CSRF (sameOrigin()) and session/auth gating already
// live correctly in src/lib/server.ts, per-route; duplicating either here
// would be redundant at best and risks drifting out of sync with the real
// checks. This middleware exists solely to (1) fail closed on the same
// ingress-configuration validation every route handler already performs via
// `forwardingHeaders()`, now applied to every request including plain page
// loads that never call the backend, and (2) set a per-request CSP nonce,
// mirroring apps/admin-web/src/middleware.ts's proven pattern exactly.
export function middleware(req: NextRequest) {
  try {
    exposureHeaders(req.headers, { environment: process.env.ENVIRONMENT,
      origin: process.env.DOCTOR_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET,
      devOtp: process.env.DEV_AUTH_OTP_ENABLED });
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Secure deployment configuration or ingress required.' },
      { status: error instanceof ExposureError ? error.status : 503, headers: { 'Cache-Control': 'no-store' } });
  }

  // Next's bootstrap/Flight scripts require a per-request nonce. A bare
  // script-src 'self' prevents hydration (the app becomes a native GET form).
  const nonce = Buffer.from(crypto.randomUUID()).toString('base64');
  const dev = process.env.NODE_ENV !== 'production';
  const csp = ["default-src 'self'", `script-src 'self' 'nonce-${nonce}'${dev ? " 'unsafe-eval'" : ''}`,
    "style-src 'self' 'unsafe-inline'", "img-src 'self' data: blob:", "font-src 'self'",
    `connect-src 'self'${dev ? ' ws://127.0.0.1:3001' : ''}`, "object-src 'none'",
    "base-uri 'none'", "frame-ancestors 'none'", "form-action 'self'"].join('; ');
  const requestHeaders = new Headers(req.headers);
  requestHeaders.set('x-nonce', nonce);
  requestHeaders.set('Content-Security-Policy', csp);
  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set('Content-Security-Policy', csp);
  return response;
}

export const config = {
  // Skip static assets and Next internals; apply to every page and API route otherwise.
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
