import test from 'node:test';
import assert from 'node:assert/strict';
import { rxBlockedReason, rxCandidates, rxClosingSoon, rxCorrectable, rxDispatchAllowed, rxHoursLeft } from '../src/lib/contracts.ts';
import { observeServerDate, serverNow, setServerClockOffset } from '../src/lib/server-clock.ts';
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

test('the server decision wins over the local rule', () => {
  // A video both apps joined: still CONFIRMED, but started.
  assert.equal(rxDispatchAllowed(apt('v', 'CONFIRMED', { rx: { allowed: true, reason: 'OK' } }), now), true);
  // A chat inside its window that the patient never wrote in: not started.
  const chat = apt('c', 'CONFIRMED', { modality: 'CHAT', start_time: iso(0.1), rx: { allowed: false, reason: 'NOT_STARTED', message: 'Not started yet.' } });
  assert.equal(rxDispatchAllowed(chat, now), false);
  assert.equal(rxBlockedReason(chat, now), 'Not started yet.');
  // The server's deadline, not one computed from the completion time.
  const done = apt('d', 'COMPLETED', { completed_at: iso(-2), rx: { allowed: true, reason: 'OK', closes_at: iso(0.5) } });
  assert.equal(rxHoursLeft(done, now), 1);
  assert.equal(rxClosingSoon(done, now), true);
  assert.equal(rxDispatchAllowed(done, now + 3600000), false);
});

test('deadlines follow the server clock from the Date header', () => {
  const received = Date.parse('2026-11-24T10:00:00Z');
  observeServerDate('Tue, 24 Nov 2026 12:00:00 GMT', received);
  assert.ok(Math.abs(serverNow() - Date.now() - 2 * 3600000) < 1000);
  observeServerDate('garbage');
  assert.ok(Math.abs(serverNow() - Date.now() - 2 * 3600000) < 1000, 'a malformed header changes nothing');
  setServerClockOffset(0);
});

test('a sent prescription takes correction notes for 7 days', () => {
  assert.equal(rxCorrectable(apt('a', 'COMPLETED', { clinical_outcome: 'COMPLETED_WITH_RX', completed_at: iso(-30) }), now), true);
  assert.equal(rxCorrectable(apt('b', 'COMPLETED', { clinical_outcome: 'COMPLETED_WITH_RX', completed_at: iso(-24 * 8) }), now), false);
  assert.equal(rxCorrectable(apt('c', 'COMPLETED', { clinical_outcome: 'COMPLETED_NO_RX', completed_at: iso(-2) }), now), false);
});
