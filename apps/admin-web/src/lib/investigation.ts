import type { InvestigationEvent } from './api';

export type TimelineEvent = InvestigationEvent & { source: string };

/** Both ledgers retain their provenance; equal IDs across ledgers are separate evidence. */
export function investigationEvents(trace: InvestigationEvent[], audit: InvestigationEvent[]): TimelineEvent[] {
  const timestamp = (event: InvestigationEvent) => {
    const parsed = Date.parse(event.event_time_server || event.created_at || '');
    const fraction=(event.event_time_server || event.created_at || '').match(/\.(\d+)(?:Z|[+-])/i)?.[1] || '';
    return { milliseconds:Number.isFinite(parsed) ? parsed : Number.MAX_SAFE_INTEGER, microseconds:Number(fraction.padEnd(6,'0').slice(3,6)) };
  };
  return [
    ...trace.map(event => ({ ...event, source: 'Temporary consultation trace' })),
    ...audit.map(event => ({ ...event, source: 'Permanent audit' })),
  ].sort((a, b) => {
    const left=timestamp(a),right=timestamp(b);
    return left.milliseconds-right.milliseconds || left.microseconds-right.microseconds || a.id.localeCompare(b.id) || a.source.localeCompare(b.source);
  });
}
