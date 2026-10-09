import { NextRequest } from 'next/server';
import { handleAuth } from '../../../../lib/server';
export const dynamic = 'force-dynamic';
export async function POST(req: NextRequest, { params }: { params: Promise<{ action: string }> }) { return handleAuth(req, (await params).action); }
