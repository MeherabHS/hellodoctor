// Every HelloDoctor surface renders the fixed Africa/Nairobi wall-clock
// (single-country deployment). This module only *labels* that fact: backend
// storage stays UTC and booking boundaries are decided server-side.
export const EAT_NOTE = 'EAT (UTC+3)';

export function eatTime(value: string | null | undefined): string {
  return value
    ? new Date(value).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit', hour12: true, timeZone: 'Africa/Nairobi' }) + ' EAT'
    : '—';
}

export function eatRange(start: string, end: string): string {
  const fmt = (v: string) => new Date(v).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit', hour12: true, timeZone: 'Africa/Nairobi' });
  return `${fmt(start)} – ${fmt(end)} EAT`;
}
