import { NextRequest, NextResponse } from 'next/server';
import { withAdminSession, backendUrl, backendHeaders } from '@/lib/session';

// Doctor verification documents (Doctor Web redesign, 2026-11-14). The JSON proxy
// reads bodies as text, so files stream through this route instead. The backend
// authorizes (Admin only), audits the view and checks the stored hash.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export async function GET(req: NextRequest) {
  const doctorId = req.nextUrl.searchParams.get('doctor_id') ?? '';
  const documentId = req.nextUrl.searchParams.get('document_id') ?? '';
  if (!UUID.test(doctorId) || !UUID.test(documentId)) {
    return NextResponse.json({ success: false, error: 'A valid doctor and document are required.' }, { status: 400, headers: { 'Cache-Control': 'no-store' } });
  }
  let res: Response;
  try {
    res = await withAdminSession(async token => fetch(`${backendUrl()}/api/v1/admin/doctors/${doctorId}/verification-documents/${documentId}`, {
      headers: { ...(await backendHeaders()), Authorization: `Bearer ${token}`, 'X-Request-Id': crypto.randomUUID(), 'X-Client-Type': 'ADMIN_WEB' },
      cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(30000),
    }));
  } catch {
    return NextResponse.json({ success: false, error: 'Backend unreachable.' }, { status: 502, headers: { 'Cache-Control': 'no-store' } });
  }
  const type = res.headers.get('Content-Type')?.split(';')[0] ?? '';
  if (!res.ok || !res.body || !['image/jpeg', 'application/pdf'].includes(type)) {
    return NextResponse.json({ success: false, error: 'Document unavailable.' }, { status: res.ok ? 502 : res.status, headers: { 'Cache-Control': 'no-store' } });
  }
  return new NextResponse(res.body, { status: 200, headers: { 'Content-Type': type, 'Content-Disposition': 'inline', 'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff' } });
}
