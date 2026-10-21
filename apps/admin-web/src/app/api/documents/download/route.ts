import { NextRequest, NextResponse } from 'next/server';
import { getAccessToken, backendUrl, backendHeaders } from '@/lib/session';

// Streams a real clinical document (prescription/intake image) to the
// browser without the browser ever talking to the Rust backend directly —
// the same "proxy, never direct" invariant src/app/api/proxy/[...path]/route.ts
// already documents. Two real backend calls, no new backend functionality:
// 1. GET /appointments/:id/documents/:document_id — already Admin-accessible
//    (confirmed by reading prescription_handlers::authorized_document
//    directly), mints a short-lived, single-use download grant.
// 2. GET /documents/download?token=... — redeems that grant (GETDEL,
//    single-use) and returns the actual file bytes.
// This route exists because a `<a href>`/`window.open` to the backend's own
// origin would both break that invariant and likely be blocked by this
// app's own CSP (`connect-src 'self'`) and the backend's ingress checks.
export async function GET(req: NextRequest) {
  const accessToken = await getAccessToken();
  if (!accessToken) {
    return NextResponse.json({ success: false, error: 'Not authenticated.' }, { status: 401 });
  }

  const appointmentId = req.nextUrl.searchParams.get('appointment_id');
  const documentId = req.nextUrl.searchParams.get('document_id');
  if (!appointmentId || !documentId) {
    return NextResponse.json({ success: false, error: 'appointment_id and document_id are required.' }, { status: 400 });
  }

  const headers = {
    ...(await backendHeaders()),
    Authorization: `Bearer ${accessToken}`,
    'X-Request-Id': crypto.randomUUID(),
    'X-Client-Type': 'ADMIN_WEB',
  };

  let grantRes: Response;
  try {
    grantRes = await fetch(
      `${backendUrl()}/api/v1/appointments/${encodeURIComponent(appointmentId)}/documents/${encodeURIComponent(documentId)}`,
      { headers, cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(15000) },
    );
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502 });
  }
  const grantPayload = await grantRes.json().catch(() => null);
  if (!grantRes.ok || !grantPayload?.success) {
    const message = grantPayload?.error?.message || 'Document not found or access denied.';
    return NextResponse.json({ success: false, error: message }, { status: grantRes.status || 502 });
  }

  let fileRes: Response;
  try {
    fileRes = await fetch(`${backendUrl()}${grantPayload.data.download_url}`, {
      headers, cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(30000),
    });
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502 });
  }
  if (!fileRes.ok || !fileRes.body) {
    return NextResponse.json({ success: false, error: 'Document access expired or invalid.' }, { status: fileRes.status || 502 });
  }

  return new NextResponse(fileRes.body, {
    status: 200,
    headers: {
      'Content-Type': fileRes.headers.get('Content-Type') || 'application/octet-stream',
      'Content-Disposition': 'inline',
      'Cache-Control': 'no-store',
    },
  });
}
