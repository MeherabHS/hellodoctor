import { NextRequest } from 'next/server';
import { handleArticleCover } from '../../../../lib/server';
export const dynamic = 'force-dynamic';
export async function GET(_req: NextRequest, { params }: { params: Promise<{ id: string }> }) { return handleArticleCover((await params).id); }
