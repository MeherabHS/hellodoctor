import { NextResponse } from 'next/server';
import { challengeRequired } from '@/lib/turnstile';

export const dynamic = 'force-dynamic';

export function GET() {
  return NextResponse.json({ required: challengeRequired(process.env.ENVIRONMENT),
    siteKey: process.env.TURNSTILE_SITE_KEY ?? '' }, { headers: { 'Cache-Control': 'private, no-store' } });
}
