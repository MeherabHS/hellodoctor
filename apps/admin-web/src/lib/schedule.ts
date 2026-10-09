// Match Doctor Web's scheduling clock regardless of the operator's device zone.
export const SCHEDULE_TIME_ZONE = 'Africa/Nairobi';
export function scheduleDay(value: string) {
  return new Intl.DateTimeFormat('en-CA', { timeZone: SCHEDULE_TIME_ZONE, year: 'numeric', month: '2-digit', day: '2-digit' }).format(new Date(value));
}
export function scheduleLabel(value: string) {
  return new Date(value).toLocaleString('en-KE', { timeZone: SCHEDULE_TIME_ZONE, year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
}
