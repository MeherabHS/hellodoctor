import { NextRequest, NextResponse } from 'next/server';
import { readFileSync } from 'node:fs';
import { withAdminSession, backendUrl, backendHeaders } from '@/lib/session';
import { safeIncident, sanitizedStack } from '@/lib/client-incidents';
import packageInfo from '../../../../package.json';

// Mirrors apps/doctor-web/src/lib/server.ts's `handleDiagnostics` exactly:
// same-origin check, require an active session cookie, allow-list the exact
// client-sent fields, re-derive the safe payload server-side (never trust the
// client's own message/error_type pairing), then forward to the backend's
// already-Admin-authorized `/api/v1/client-errors` with this app's own
// client/app_version/build_number convention.
function buildNumber() {
  if (process.env.ADMIN_WEB_BUILD_NUMBER) return process.env.ADMIN_WEB_BUILD_NUMBER;
  try { return readFileSync('.next/BUILD_ID', 'utf8').trim(); }
  catch { return 'development'; }
}
function sameOrigin(req: NextRequest) {
  const origin = req.headers.get('origin');
  const site = req.headers.get('sec-fetch-site');
  const expected = process.env.ADMIN_WEB_ORIGIN;
  return !!expected && origin === expected && (!site || site === 'same-origin' || site === 'none');
}
function failure(status: number, message: string) {
  return NextResponse.json({ success: false, error: message }, { status, headers: { 'Cache-Control': 'private, no-store' } });
}
const ALLOWED = ['client', 'error_type', 'message', 'stack_trace', 'screen', 'operation', 'platform', 'occurred_at', 'backend_reference'];

export async function POST(req: NextRequest) {
  if (!sameOrigin(req)) return failure(403, 'Request origin is not permitted.');
  let data: unknown;
  try { data = await req.json(); } catch { return failure(422, 'A valid incident is required.'); }
  if (!data || typeof data !== 'object' || Array.isArray(data) || Object.keys(data).some((k) => !ALLOWED.includes(k))) {
    return failure(422, 'Unsupported incident fields.');
  }
  const body = data as Record<string, unknown>;
  if (['error_type', 'message', 'stack_trace', 'screen', 'operation', 'occurred_at'].some((k) => typeof body[k] !== 'string')
    || (body.message as string).length > 2000 || (body.stack_trace as string).length > 6000
    || (body.error_type as string).length > 64 || (body.screen as string).length > 32 || (body.operation as string).length > 40) {
    return failure(422, 'Incident exceeds field limits.');
  }

  const safe = safeIncident(undefined,
    { screen: body.screen as string, operation: body.operation as string, backendReference: body.backend_reference as string | undefined },
    body.error_type as string);
  const payload = { ...safe, stack_trace: sanitizedStack(body.stack_trace), app_version: packageInfo.version, build_number: buildNumber() };

  let res: Response;
  try {
    res = await withAdminSession(async accessToken=>fetch(`${backendUrl()}/api/v1/client-errors`, {
      method: 'POST', cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(2500),
      headers: { ...(await backendHeaders()), 'Content-Type': 'application/json', Authorization: `Bearer ${accessToken}` },
      body: JSON.stringify(payload),
    }));
  } catch {
    return failure(503, 'Incident delivery unavailable.');
  }
  const raw = await res.text();
  return new NextResponse(raw, { status: res.status, headers: { 'Content-Type': 'application/json', 'Cache-Control': 'private, no-store' } });
}
