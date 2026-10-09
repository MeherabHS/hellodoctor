import { NextRequest } from 'next/server';
import { handleProxy } from '../../../../lib/server';
export const dynamic = 'force-dynamic';
async function handle(req: NextRequest, { params }: { params: Promise<{ path: string[] }> }) { return handleProxy(req, (await params).path); }
export { handle as GET, handle as POST, handle as PATCH, handle as PUT, handle as DELETE };
