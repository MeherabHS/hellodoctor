import test from 'node:test';
import assert from 'node:assert/strict';
import { eatRange, eatTime, EAT_NOTE } from '../src/lib/eat.ts';

test('times render as Africa/Nairobi wall-clock labelled EAT, whatever the process timezone', () => {
  // 18:10Z is 21:10 in Nairobi (UTC+3, no DST).
  assert.equal(eatTime('2026-11-11T18:10:00Z'), '9:10 PM EAT');
  assert.equal(eatRange('2026-11-11T18:10:00Z', '2026-11-11T18:20:00Z'), '9:10 PM – 9:20 PM EAT');
  assert.equal(EAT_NOTE, 'EAT (UTC+3)');
});

test('boundary instants: midnight rollover and the 21:00Z day change', () => {
  assert.equal(eatTime('2026-11-11T20:59:00Z'), '11:59 PM EAT');
  assert.equal(eatTime('2026-11-11T21:00:00Z'), '12:00 AM EAT');
  assert.equal(eatTime(null), '—');
});
