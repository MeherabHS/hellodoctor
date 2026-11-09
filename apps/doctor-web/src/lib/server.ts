import { cookies, headers } from 'next/headers';
import { exposureHeaders, developmentOtpAllowed } from './exposure';
import { NextRequest, NextResponse } from 'next/server';
import { safeBackendPath, allowedProxyQuery } from './contracts';
import { safeIncident, sanitizedStack } from './client-incidents';
import { sanitize } from './sanitize';
import { verifyChallenge, readAuthBody, AuthBodyError } from './turnstile';
import { validateAcceptedDocumentsShape } from './legalConsent';
import packageInfo from '../../package.json';
import { readFileSync } from 'node:fs';

function webBuildNumber() {
  if (process.env.DOCTOR_WEB_BUILD_NUMBER) return process.env.DOCTOR_WEB_BUILD_NUMBER;
  try { return readFileSync('.next/BUILD_ID','utf8').trim(); }
  catch { return 'development'; }
}

const ACCESS = 'hd_doctor_access';
const REFRESH = 'hd_doctor_refresh';
const CHALLENGE = 'hd_doctor_challenge';
const SCOPE = 'hd_doctor_scope';
const cookieOptions = { httpOnly: true, secure: process.env.NODE_ENV === 'production', sameSite: 'strict' as const, path: '/api' };
const rotations = new Map<string, Promise<{ access_token: string; refresh_token: string } | null>>();
const revoked = new Map<string, number>();
class PayloadLimit extends Error {}
async function readBody(req: NextRequest, maximum: number) {
  if (Number(req.headers.get('content-length')) > maximum) throw new PayloadLimit();
  const reader = req.body?.getReader();
  if (!reader) return '';
  const chunks: Uint8Array[] = [];let length = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > maximum) { await reader.cancel(); throw new PayloadLimit(); }
    chunks.push(value);
  }
  return Buffer.concat(chunks).toString('utf8');
}
function isRevoked(scope: string) {
  for (const [key, until] of revoked) if (until < Date.now()) revoked.delete(key);
  return revoked.has(scope);
}
export async function clearSession() { for (const name of [ACCESS, REFRESH, CHALLENGE, SCOPE]) (await cookies()).delete({ name, path: '/api' }); }
async function setTokens(data: { access_token: string; refresh_token: string }) {
  (await cookies()).set(ACCESS, data.access_token, { ...cookieOptions, maxAge: 900 });
  (await cookies()).set(REFRESH, data.refresh_token, { ...cookieOptions, maxAge: 604800 });
}
function backend() {
  const configured = process.env.BACKEND_URL;
  if (!configured) throw new Error('BACKEND_CONFIGURATION_REQUIRED');
  const url = new URL(configured);
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.hash) throw new Error('BACKEND_CONFIGURATION_REQUIRED');
  return url.origin;
}
export function sameOrigin(req: NextRequest) {
  const origin = req.headers.get('origin');
  const site = req.headers.get('sec-fetch-site');
  const expected = process.env.DOCTOR_WEB_ORIGIN;
  return !!expected && origin === expected && (!site || site === 'same-origin' || site === 'none');
}
function failure(status: number, code: string, message: string, requestId?: string) {
  return NextResponse.json({ success: false, error: { code, message }, meta: { request_id: requestId ?? crypto.randomUUID() } }, { status, headers: { 'Cache-Control': 'private, no-store' } });
}
async function forwardingHeaders() {
  return exposureHeaders(new Headers(await headers()), { environment: process.env.ENVIRONMENT,
    origin: process.env.DOCTOR_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET, devOtp: process.env.DEV_AUTH_OTP_ENABLED });
}
async function upstream(path: string, method = 'GET', body?: string, token?: string, key?: string) {
  return fetch(`${backend()}/api/v1/${path}`, { method, body, cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout((path.endsWith('prescription') || path.endsWith('attachments')) ? 60000 : 15000), headers: { ...(await forwardingHeaders()), 'Content-Type': 'application/json', 'X-Request-Id':(await headers()).get('X-Request-Id')||crypto.randomUUID(),'X-Client-Type':'DOCTOR_WEB','X-Client-Build':packageInfo.version, ...(token ? { Authorization: `Bearer ${token}` } : {}), ...(key ? { 'Idempotency-Key': key } : {}) } });
}
/// GET-only, unauthenticated by design: a prospective Doctor reads the
/// current legal documents before any session exists at all (the same
/// backend endpoint, GET /api/v1/legal/documents, that the backend's own
/// doctor_register handler already treats as the pre-session source of
/// truth for required document type/version pairs). No user input is
/// accepted here, so there is nothing to validate or forward -- this is a
/// pure passthrough of the backend's public registry response.
export async function handleLegalDocuments() {
  try {
    return responseFor(await upstream('legal/documents', 'GET'));
  } catch {
    return failure(503, 'BACKEND_UNAVAILABLE', 'Unable to reach the service. Please retry.');
  }
}
export async function handleDiagnostics(req: NextRequest) {
  if (!sameOrigin(req)) return failure(403, 'CSRF_REJECTED', 'Request origin is not permitted.');
  try {
    // No refresh/login/session mutation from best-effort error delivery.
    const jar = await cookies();
    const scope = jar.get(SCOPE)?.value;
    const token = jar.get(ACCESS)?.value;
    if (!scope || isRevoked(scope) || !token) return failure(401, 'SESSION_EXPIRED', 'Doctor session required.');
    const data = JSON.parse(await readBody(req, 16384));
    const allowed = ['client','error_type','message','stack_trace','screen','operation','platform','occurred_at','backend_reference'];
    if (!data || typeof data !== 'object' || Array.isArray(data) || Object.keys(data).some(k=>!allowed.includes(k))) return failure(422,'VALIDATION_FAILED','Unsupported incident fields.');
    if (['error_type','message','stack_trace','screen','operation','occurred_at'].some(k=>typeof data[k]!=='string') || data.message.length>2000 || data.stack_trace.length>6000 || data.error_type.length>64 || data.screen.length>32 || data.operation.length>40) return failure(422,'VALIDATION_FAILED','Incident exceeds field limits.');
    const safe = safeIncident(undefined,{screen:data.screen,operation:data.operation,backendReference:data.backend_reference},data.error_type);
    const payload = {...safe, stack_trace:sanitizedStack(data.stack_trace),
      app_version:packageInfo.version, build_number:webBuildNumber()};
    const res = await fetch(`${backend()}/api/v1/client-errors`, {method:'POST',cache:'no-store',redirect:'error',
      signal:AbortSignal.timeout(2500),headers:{...(await forwardingHeaders()),'Content-Type':'application/json',Authorization:`Bearer ${token}`},body:JSON.stringify(payload)});
    if (isRevoked(scope)) return failure(401,'SESSION_EXPIRED','Doctor session required.');
    return responseFor(res);
  } catch (error) {
    if (error instanceof PayloadLimit) return failure(413,'PAYLOAD_TOO_LARGE','Incident is too large.');
    if (error instanceof SyntaxError) return failure(422,'VALIDATION_FAILED','A valid incident is required.');
    // Never recurse or log arbitrary error data from this endpoint.
    return failure(503,'INCIDENT_UNAVAILABLE','Incident delivery unavailable.');
  }
}
async function responseFor(res: Response) {
  const raw = await res.text();
  let data;
  try { data = JSON.parse(raw); } catch {
    const status = res.ok ? 502 : res.status;
    const requestId = crypto.randomUUID();
    // Server-side diagnostic only: status, content-type and body length
    // correlate this with backend/proxy logs without ever recording the raw
    // body (which, for a non-JSON upstream rejection, can itself be an
    // unredacted stack trace or field-level detail) or any request content
    // (password, OTP, cookie, token, PII never touch this log line).
    console.error('[doctor-web] non-JSON upstream response', {
      requestId, status,
      contentType: res.headers.get('content-type'),
      bodyLength: raw.length,
    });
    const message = status === 409
      ? 'This operation conflicts with an existing submission. Refresh and retry.'
      : status >= 500
        ? 'The service is temporarily unavailable. Please try again shortly.'
        : `Your request could not be processed. Please check the form and try again, or contact support with reference ${requestId} if this continues.`;
    return failure(status, `HTTP_${res.status}`, message, requestId);
  }
  if (!res.ok) {
    const status = res.status;
    return failure(status, data.error?.code ?? `HTTP_${status}`, status >= 500 ? (status === 503 ? (String(data.error?.message).includes('STORAGE_UNAVAILABLE') ? 'STORAGE_UNAVAILABLE' : 'Service temporarily unavailable. Please retry.') : 'Unable to complete this request. Use the reference ID when contacting support.') : data.error?.message ?? 'Request could not be completed.', data.meta?.request_id);
  }
  return NextResponse.json(sanitize(data), { status: res.status, headers: { 'Cache-Control': 'private, no-store', 'X-Request-Id':res.headers.get('X-Request-Id')||'' } });
}
async function access() {
  const scope = (await cookies()).get(SCOPE)?.value;
  if (!scope || isRevoked(scope)) return null;
  const token = (await cookies()).get(ACCESS)?.value;
  if (token) return { token, scope };
  const refreshed = await rotate(scope);
  return refreshed ? { token: refreshed.access_token, scope } : null;
}
async function rotate(scope: string) {
  const refresh = (await cookies()).get(REFRESH)?.value;
  if (!refresh || isRevoked(scope)) return null;
  let pending = rotations.get(refresh);
  if (!pending) {
    pending = (async () => {
      const res = await upstream('auth/refresh', 'POST', JSON.stringify({ refresh_token: refresh }));
      if (!res.ok) return null;
      const data = (await res.json()).data;
      return { access_token: data.access_token as string, refresh_token: data.refresh_token as string };
    })();
    rotations.set(refresh, pending);
  }
  try {
    const data = await pending;
    if (!data || isRevoked(scope)) { await clearSession(); return null; }
    await setTokens(data);
    return data;
  } finally {
    // Keep a short result grace period for concurrent requests with the old cookie.
    setTimeout(() => rotations.delete(refresh), 5000).unref();
  }
}
async function invalidateCurrentScope() {
  const scope = (await cookies()).get(SCOPE)?.value;
  if (scope) revoked.set(scope, Date.now() + 604800000);
}
export async function handleAuth(req: NextRequest, action: string) {
  if (!sameOrigin(req)) return failure(403, 'CSRF_REJECTED', 'Request origin is not permitted.');
  try {
    if (action === 'logout') {
      const refresh = (await cookies()).get(REFRESH)?.value;
      await invalidateCurrentScope();
      await clearSession();
      if (refresh) {
        try { const res = await upstream('auth/logout', 'POST', JSON.stringify({ refresh_token: refresh })); if (!res.ok) return failure(503, 'LOGOUT_REVOCATION_UNCONFIRMED', 'Signed out locally. Server revocation could not be confirmed.'); }
        catch { return failure(503, 'LOGOUT_REVOCATION_UNCONFIRMED', 'Signed out locally. Server revocation could not be confirmed.'); }
      }
      return NextResponse.json({ success: true, data: { logged_out: true } });
    }
    const body = await readAuthBody(req);
    if (!body || typeof body !== 'object' || Array.isArray(body)) return failure(422, 'VALIDATION_FAILED', 'A valid form is required.');
    if (action === 'login' || action === 'register') {
      if (typeof body.email !== 'string' || !body.email || body.email.length > 254 ||
          typeof body.password !== 'string' || !body.password || body.password.length > 128) {
        return failure(422, 'VALIDATION_FAILED', 'A valid email and password are required.');
      }
      const challenge = await verifyChallenge(body.turnstile_token, `doctor_${action}`, {
        environment: process.env.ENVIRONMENT, origin: process.env.DOCTOR_WEB_ORIGIN,
        secret: process.env.TURNSTILE_SECRET_KEY,
      });
      if (!challenge.ok) {
        const response = failure(challenge.status, 'SIGN_IN_UNAVAILABLE', "We couldn't complete sign-in. Please try again.");
        if (challenge.status === 503) response.headers.set('Retry-After', '5');
        return response;
      }
      delete body.turnstile_token;
    }
    if (action === 'register') {
      const allowed = ['full_name','license_number','phone_number','email','password','primary_specialty','experience_years','current_hospital','qualifications','residential_address','accepted_documents'];
      if (Object.keys(body).some(key => !allowed.includes(key))) return failure(422, 'VALIDATION_FAILED', 'Registration contains unsupported fields.');
      if (!validateAcceptedDocumentsShape(body.accepted_documents)) {
        return failure(422, 'VALIDATION_FAILED', 'Every current required legal document must be accepted.');
      }
      return responseFor(await upstream('auth/doctor/register', 'POST', JSON.stringify(body)));
    }
    if (action === 'login') {
      // Revoke the previous account before starting a new interactive identity boundary.
      const previous = (await cookies()).get(REFRESH)?.value;
      await invalidateCurrentScope();
      await clearSession();
      if (previous) {
        const previousLogout = await upstream('auth/logout', 'POST', JSON.stringify({ refresh_token: previous }));
        if (!previousLogout.ok) return failure(503, 'LOGOUT_REVOCATION_UNCONFIRMED', 'Signed out locally. Previous session revocation could not be confirmed.');
      }
      const res = await upstream('auth/doctor/login', 'POST', JSON.stringify({ email: String(body.email ?? '').trim().toLowerCase(), password: body.password }));
      if (!res.ok) return responseFor(res);
      const data = (await res.json()).data;
      (await cookies()).set(CHALLENGE, data.session_token, { ...cookieOptions, maxAge: 300 });
      // MFA always visible; never silently bypass it. Development code is transient and labelled.
      return NextResponse.json({ success: true, data: { requires_otp: true, development_otp: developmentOtpAllowed({ environment: process.env.ENVIRONMENT, devOtp: process.env.DEV_AUTH_OTP_ENABLED }) ? data.debug_otp_code ?? null : null } });
    }
    if (action === 'verify') {
      const challenge = (await cookies()).get(CHALLENGE)?.value;
      if (!challenge) return failure(401, 'MFA_EXPIRED', 'Sign in again to request a verification code.');
      const res = await upstream('auth/doctor/verify-otp', 'POST', JSON.stringify({ session_token: challenge, otp_code: body.otp_code }));
      if (!res.ok) return responseFor(res);
      const data = (await res.json()).data;
      await setTokens(data);
      (await cookies()).delete({ name: CHALLENGE, path: '/api' });
      const scope = crypto.randomUUID();
      (await cookies()).set(SCOPE, scope, { ...cookieOptions, maxAge: 604800 });
      return NextResponse.json({ success: true, data: { signed_in: true, scope } });
    }
    return failure(404, 'NOT_FOUND', 'Action not found.');
  } catch (error) { if (error instanceof AuthBodyError) return failure(error.status, 'INVALID_FORM', 'A valid form is required.'); if (error instanceof PayloadLimit) return failure(413, 'PAYLOAD_TOO_LARGE', 'Form is too large.'); if (error instanceof SyntaxError) return failure(422, 'VALIDATION_FAILED', 'A valid form is required.'); return failure(503, 'BACKEND_UNAVAILABLE', 'Unable to reach the service. Please retry.'); }
}
export async function handleProxy(req: NextRequest, segments: string[]) {
  const path = segments.join('/');
  if (!safeBackendPath(path, req.method)) return failure(404, 'NOT_FOUND', 'Endpoint not available.');
  if (req.method !== 'GET' && !sameOrigin(req)) return failure(403, 'CSRF_REJECTED', 'Request origin is not permitted.');
  try {
    let session = await access();
    if (!session) return failure(401, 'SESSION_EXPIRED', 'Please sign in again.');
    if (!allowedProxyQuery(path,req.nextUrl.searchParams)) return failure(422, 'VALIDATION_FAILED', 'Unsupported query field.');
    const query = req.nextUrl.search;
    const body = req.method === 'GET' || req.method === 'DELETE' ? undefined : await readBody(req, 10 * 1024 * 1024);
    if (body && Buffer.byteLength(body) > 10 * 1024 * 1024) return failure(413, 'PAYLOAD_TOO_LARGE', 'Prescription page is too large.');
    const browserKey = req.headers.get('Idempotency-Key') ?? undefined;
    if (browserKey && !/^[0-9a-f-]{36}$/i.test(browserKey)) return failure(422, 'VALIDATION_FAILED', 'Invalid request key.');
    const requestKey = browserKey ? `${session.scope}:${browserKey}` : undefined;
    let res = await upstream(path + query, req.method, body, session.token, requestKey);
    if (res.status === 401) {
      const data = await rotate(session.scope);
      if (!data) { await clearSession(); return failure(401, 'SESSION_EXPIRED', 'Please sign in again.'); }
      session = { token: data.access_token, scope: session.scope };
      res = await upstream(path + query, req.method, body, session.token, requestKey);
    }
    if (isRevoked(session.scope)) return failure(401, 'SESSION_EXPIRED', 'Please sign in again.');
    const response = await responseFor(res);
    response.headers.set('X-Doctor-Scope', session.scope);
    return response;
  } catch (error) { if (error instanceof PayloadLimit) return failure(413, 'PAYLOAD_TOO_LARGE', 'Prescription page is too large.'); return failure(503, 'BACKEND_UNAVAILABLE', 'Unable to reach the service. Please retry.'); }
}
export async function handleDocument(req: NextRequest, appointment: string, document: string) {
  if (!/^[0-9a-f-]{36}$/i.test(appointment) || !/^[0-9a-f-]{36}$/i.test(document)) return failure(404, 'NOT_FOUND', 'Document not found.');
  try {
    let session = await access();
    if (!session) return failure(401, 'SESSION_EXPIRED', 'Please sign in again.');
    const current = req.nextUrl.searchParams.get('current_appointment_id') ?? appointment;
    if (!/^[0-9a-f-]{36}$/i.test(current)) return failure(422, 'VALIDATION_FAILED', 'Invalid appointment context.');
    let grant = await upstream(`appointments/${appointment}/documents/${document}?current_appointment_id=${current}`, 'GET', undefined, session.token);
    if (grant.status === 401) {
      const refreshed = await rotate(session.scope);
      if (!refreshed) { await clearSession(); return failure(401, 'SESSION_EXPIRED', 'Please sign in again.'); }
      session = { token: refreshed.access_token, scope: session.scope };
      grant = await upstream(`appointments/${appointment}/documents/${document}?current_appointment_id=${current}`, 'GET', undefined, session.token);
    }
    if (!grant.ok) return responseFor(grant);
    const path = (await grant.json()).data.download_url;
    if (!/^\/api\/v1\/documents\/download\?token=[a-f0-9]{64}$/.test(path)) return failure(502, 'DOCUMENT_UNAVAILABLE', 'Document access is unavailable.');
    const file = await fetch(backend() + path, { cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000), headers: await forwardingHeaders() });
    if (!file.ok) return responseFor(file);
    if (isRevoked(session.scope)) return failure(401, 'SESSION_EXPIRED', 'Please sign in again.');
    const mime = file.headers.get('content-type')?.split(';')[0] ?? '';
    if (!['image/png','image/jpeg','image/webp','application/pdf'].includes(mime)) return failure(422, 'DOCUMENT_TYPE_UNSUPPORTED', 'This file type cannot be displayed.');
    return new NextResponse(await file.arrayBuffer(), { headers: { 'Content-Type': mime, 'Cache-Control': 'private, no-store', 'X-Content-Type-Options': 'nosniff', 'Content-Disposition': 'attachment; filename="clinical-document"', 'X-Doctor-Scope': session.scope } });
  } catch { return failure(503, 'DOCUMENT_UNAVAILABLE', 'Document service is temporarily unavailable.'); }
}
