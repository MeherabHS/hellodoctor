import { NextRequest } from 'next/server';
import { handleDiagnostics } from '../../../lib/server';
export async function POST(req: NextRequest) { return handleDiagnostics(req); }
