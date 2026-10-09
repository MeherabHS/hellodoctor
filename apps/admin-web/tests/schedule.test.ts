import test from 'node:test';
import assert from 'node:assert/strict';
import { scheduleDay, scheduleLabel } from '../src/lib/schedule.ts';

test('Admin renders the same East Africa schedule clock as Doctor Web on a Dhaka device', () => {
  const previous = process.env.TZ;
  process.env.TZ = 'Asia/Dhaka';
  try {
    const value = '2026-10-09T05:00:00Z';
    assert.equal(scheduleDay(value), '2026-10-09');
    assert.equal(scheduleLabel(value), new Date(value).toLocaleString('en-KE', {
      timeZone: 'Africa/Nairobi', year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    }));
    assert.match(scheduleLabel(value), /08:00/);
    assert.match(scheduleLabel('2026-12-01T10:00:00Z'), /13:00/);
  } finally {
    if (previous === undefined) delete process.env.TZ;
    else process.env.TZ = previous;
  }
});

test('date filtering uses the Doctor schedule date across UTC midnight', () => {
  assert.equal(scheduleDay('2026-10-08T21:30:00Z'), '2026-10-09');
  assert.equal(scheduleDay('2026-10-08T20:59:00Z'), '2026-10-08');
});
