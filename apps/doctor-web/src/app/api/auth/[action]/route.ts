import { NextRequest, NextResponse } from 'next/server';
import { handleAuth, handleLegalDocuments } from '../../../../lib/server';
export const dynamic = 'force-dynamic';
export async function POST(req: NextRequest, { params }: { params: Promise<{ action: string }> }) { return handleAuth(req, (await params).action); }
export async function GET(_req: NextRequest, { params }: { params: Promise<{ action: string }> }) {
  if ((await params).action !== 'legal-documents') {
    return NextResponse.json({ success: false, error: { code: 'NOT_FOUND', message: 'Action not found.' } }, { status: 404 });
  }
  return handleLegalDocuments();
}
