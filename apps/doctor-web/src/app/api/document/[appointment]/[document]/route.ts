import { NextRequest } from 'next/server';
import { handleDocument } from '../../../../../lib/server';
export const dynamic = 'force-dynamic';
export async function GET(req: NextRequest, { params }: { params: Promise<{ appointment: string; document: string }> }) { const ids = await params; return handleDocument(req, ids.appointment, ids.document); }
