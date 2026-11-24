import test from 'node:test';
import assert from 'node:assert/strict';
import { rxBlockedReason, rxCandidates, rxDispatchAllowed, rxHoursLeft } from '../src/lib/contracts.ts';
import type { Appointment } from '../src/lib/contracts.ts';

// Product owner, 2026-11-24: a prescription only once the consultation has
// started, or within 24 hours after it was completed with no decision yet.
const now = Date.parse('2026-11-24T10:00:00Z');
const iso = (hours: number) => new Date(now + hours * 3600000).toISOString();
const apt = (id: string, status: string, extra: Partial<Appointment> = {}) => ({
  id, appointment_number: `AM-${id}`, patient_display_name: id, modality: 'VIDEO', status,
  start_time: iso(-1), end_time: iso(-0.5), clinical_outcome: null, completed_at: null, ...extra,
} as Appointment);

test('video: only once someone has joined', () => {
  assert.equal(rxDispatchAllowed(apt('a', 'IN_CONSULTATION'), now), true);
  assert.equal(rxDispatchAllowed(apt('b', 'CONFIRMED'), now), false);
  assert.equal(rxDispatchAllowed(apt('c', 'WAITING'), now), false);
  assert.match(rxBlockedReason(apt('b', 'CONFIRMED'), now), /once the consultation has started/);
});

test('chat: once its window opens, 15 minutes before the start', () => {
  assert.equal(rxDispatchAllowed(apt('a', 'CONFIRMED', { modality: 'CHAT', start_time: iso(10 / 60) }), now), true);
  assert.equal(rxDispatchAllowed(apt('b', 'CONFIRMED', { modality: 'CHAT', start_time: iso(2) }), now), false);
});

test('completed: 24 hours, no decision, never again once decided', () => {
  assert.equal(rxDispatchAllowed(apt('a', 'COMPLETED', { clinical_outcome: 'REFERRED', completed_at: iso(-2) }), now), true);
  assert.equal(rxHoursLeft(apt('a', 'COMPLETED', { completed_at: iso(-2) }), now), 22);
  assert.equal(rxDispatchAllowed(apt('b', 'COMPLETED', { completed_at: iso(-25) }), now), false);
  assert.match(rxBlockedReason(apt('b', 'COMPLETED', { completed_at: iso(-25) }), now), /window .* has closed/);
  assert.equal(rxDispatchAllowed(apt('c', 'COMPLETED', { clinical_outcome: 'COMPLETED_WITH_RX', completed_at: iso(-1) }), now), false);
  for (const status of ['PENDING_PAYMENT', 'CANCELLED', 'EXPIRED']) assert.equal(rxDispatchAllowed(apt('d', status), now), false);
});

test('the Rx list holds completed consultations awaiting a prescription', () => {
  const rows = [
    apt('live', 'IN_CONSULTATION'), apt('booked', 'CONFIRMED'),
    apt('recent', 'COMPLETED', { completed_at: iso(-2) }), apt('older', 'COMPLETED', { completed_at: iso(-20) }),
    apt('decided', 'COMPLETED', { clinical_outcome: 'COMPLETED_NO_RX', completed_at: iso(-1) }), apt('stale', 'COMPLETED', { completed_at: iso(-30) }),
  ];
  assert.deepEqual(rxCandidates(rows, null, now).map(a => a.id), ['older', 'recent']);
  // Opened from its consultation room: the started one is offered too.
  assert.deepEqual(rxCandidates(rows, rows[0], now).map(a => a.id), ['live', 'older', 'recent']);
  // ...but never a consultation that has not started.
  assert.deepEqual(rxCandidates(rows, rows[1], now).map(a => a.id), ['older', 'recent']);
});
