import { NextResponse } from 'next/server';
export const dynamic = 'force-dynamic';
export function GET() {
  const simulations = ['development', 'test', 'staging'].includes(process.env.ENVIRONMENT ?? '') &&
    process.env.DEV_SIMULATION_ENABLED === 'true';
  return NextResponse.json({ simulations }, { headers: { 'Cache-Control': 'private, no-store' } });
}
