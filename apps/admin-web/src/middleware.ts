import { NextRequest, NextResponse } from 'next/server';
import { exposureHeaders, ExposureError } from './lib/exposure';

// UX-level gate only — redirects an obviously-unauthenticated browser to
// /login before it ever renders the dashboard shell. This is NOT the real
// security boundary (middleware can't verify a JWT's signature without
// pulling in the Ed25519 public key and the `jsonwebtoken`-equivalent logic,
// and shouldn't need to); the actual boundary is the Rust backend rejecting
// every proxied request that lacks a valid bearer token, enforced in
// `src/app/api/proxy/[...path]/route.ts` on every single call regardless of
// what this middleware does or doesn't catch.
const SESSION_MARKER = 'hd_admin_session';
const PUBLIC_PATHS = ['/login', '/api/login', '/api/bot-challenge'];

export function middleware(req: NextRequest) {
  try {
    exposureHeaders(req.headers, { environment: process.env.ENVIRONMENT,
      origin: process.env.ADMIN_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET,
      devOtp: process.env.DEV_AUTH_OTP_ENABLED });
  } catch (error) {
    return NextResponse.json({ success: false, error: 'Secure deployment configuration or ingress required.' },
      { status: error instanceof ExposureError ? error.status : 503, headers: { 'Cache-Control': 'no-store' } });
  }

  const { pathname } = req.nextUrl;
  if (!['GET', 'HEAD', 'OPTIONS'].includes(req.method) &&
      req.headers.get('origin') !== process.env.ADMIN_WEB_ORIGIN) {
    return NextResponse.json({ success: false, error: 'Request origin is not permitted.' }, { status: 403 });
  }
  // Next's bootstrap/Flight scripts require a per-request nonce. A bare
  // script-src 'self' prevents hydration (the login becomes a native GET form).
  const nonce = Buffer.from(crypto.randomUUID()).toString('base64');
  const dev = process.env.NODE_ENV !== 'production';
  const csp = ["default-src 'self'", `script-src 'self' 'nonce-${nonce}' https://challenges.cloudflare.com${dev ? " 'unsafe-eval'" : ''}`,
    "style-src 'self' 'unsafe-inline'", "img-src 'self' data:", "font-src 'self'",
    `connect-src 'self'${dev ? ' ws://127.0.0.1:3000' : ''}`, "object-src 'none'",
    "base-uri 'none'", "frame-src https://challenges.cloudflare.com", "frame-ancestors 'none'", "form-action 'self'"].join('; ');
  const requestHeaders = new Headers(req.headers);
  requestHeaders.set('x-nonce', nonce);
  requestHeaders.set('Content-Security-Policy', csp);
  const next = () => {
    const response = NextResponse.next({ request: { headers: requestHeaders } });
    response.headers.set('Content-Security-Policy', csp);
    return response;
  };

  if (PUBLIC_PATHS.some((p) => pathname === p || pathname.startsWith(p + '/'))) {
    return next();
  }

  const hasSession = req.cookies.has(SESSION_MARKER);
  if (!hasSession && !pathname.startsWith('/api/')) {
    const loginUrl = new URL('/login', process.env.ADMIN_WEB_ORIGIN || req.url);
    return NextResponse.redirect(loginUrl);
  }

  return next();
}

export const config = {
  // Skip static assets and Next internals; apply to every page and API route otherwise.
  matcher: ['/((?!_next/static|_next/image|favicon.ico).*)'],
};
