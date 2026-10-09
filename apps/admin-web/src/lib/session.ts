// Server-only session helpers. Tokens live exclusively in HttpOnly cookies —
// they are set and read here, on the server, and never touch client-side
// JavaScript or localStorage. This is the one place that defines the cookie
// attributes; every route handler that needs a token goes through here
// rather than reading `cookies()` directly, so the security properties
// (HttpOnly, Secure in production, SameSite=strict, scoped to /api) stay
// consistent everywhere.
import { cookies, headers } from 'next/headers';
import { exposureHeaders } from './exposure';

const ACCESS_COOKIE = 'hd_admin_access';
const REFRESH_COOKIE = 'hd_admin_refresh';
const SESSION_MARKER = 'hd_admin_session';

// Matches the backend's access-token lifetime (crypto.rs::ACCESS_TOKEN_TTL_SECONDS)
// so the cookie never outlives the token it holds.
const ACCESS_TOKEN_TTL_SECONDS = 15 * 60;
// Matches the backend's refresh-token lifetime (auth_service.rs::REFRESH_TOKEN_TTL_DAYS).
const REFRESH_TOKEN_TTL_SECONDS = 7 * 24 * 60 * 60;

const baseCookieOptions = {
  httpOnly: true,
  secure: process.env.NODE_ENV === 'production',
  sameSite: 'strict' as const,
  path: '/api',
};

export async function setSessionCookies(accessToken: string, refreshToken: string) {
  const store = await cookies();
  store.set(ACCESS_COOKIE, accessToken, { ...baseCookieOptions, maxAge: ACCESS_TOKEN_TTL_SECONDS });
  store.set(REFRESH_COOKIE, refreshToken, { ...baseCookieOptions, maxAge: REFRESH_TOKEN_TTL_SECONDS });
  // Page middleware cannot see /api-scoped tokens. This non-credential marker
  // gates only the shell; every data request still requires backend JWT auth.
  store.set(SESSION_MARKER, '1', { ...baseCookieOptions, path: '/', maxAge: ACCESS_TOKEN_TTL_SECONDS });
}

export async function clearSessionCookies() {
  const store = await cookies();
  store.delete({ name: ACCESS_COOKIE, path: '/api' });
  store.delete({ name: REFRESH_COOKIE, path: '/api' });
  store.delete({ name: SESSION_MARKER, path: '/' });
}

export async function getAccessToken(): Promise<string | undefined> {
  const store = await cookies();
  return store.get(ACCESS_COOKIE)?.value;
}

export async function getRefreshToken(): Promise<string | undefined> {
  const store = await cookies();
  return store.get(REFRESH_COOKIE)?.value;
}

export function backendUrl(): string {
  const url = process.env.BACKEND_URL;
  if (!url) {
    throw new Error('BACKEND_URL is not set (see .env.local.example).');
  }
  return url;
}

export async function backendHeaders(): Promise<Record<string, string>> {
  const incoming = await headers();
  return exposureHeaders(new Headers(incoming), { environment: process.env.ENVIRONMENT,
    origin: process.env.ADMIN_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET,
    devOtp: process.env.DEV_AUTH_OTP_ENABLED });
}
