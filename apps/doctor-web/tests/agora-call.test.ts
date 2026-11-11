import test from 'node:test';
import assert from 'node:assert/strict';
import { callStatusText, callStatusTone } from '../src/lib/call-status.ts';
import { classifyAgoraErrorCode } from '../src/lib/agora-errors.ts';
import type { Appointment } from '../src/lib/contracts.ts';

const base = { id: 'a', appointment_number: 'HD-1', patient_id: 'p', patient_display_name: 'Rafiq Ahmed', doctor_lifecycle: 'ACTIVE', history_access_from: '2026-01-01T00:00:00Z', history_access_until: null, history_access_active: true, clinical_outcome: null } as const;
function appointment(overrides: Partial<Appointment>): Appointment {
  return { ...base, modality: 'VIDEO', status: 'CONFIRMED', start_time: '2026-01-01T00:00:00Z', end_time: '2026-01-01T00:30:00Z', ...overrides };
}

test('no appointment selected shows the empty state regardless of call phase', () => {
  assert.equal(callStatusText(null, false, 'idle', false), 'No consultation selected');
  assert.equal(callStatusTone('idle', false), 'slate');
});

test('an ineligible appointment is described by its own status, never by stale call phase', () => {
  assert.equal(callStatusText(appointment({ status: 'PENDING_PAYMENT' }), false, 'connected', true), 'Waiting for payment confirmation');
  assert.equal(callStatusText(appointment({ status: 'COMPLETED' }), false, 'connected', true), 'Consultation completed');
  for (const status of ['CANCELLED', 'EXPIRED', 'PREMATURE_TERMINATION', 'DISPUTED', 'REFUNDED'])
    assert.equal(callStatusText(appointment({ status }), false, 'connected', true), 'Call ended');
  assert.equal(callStatusText(appointment({ status: 'WAITING' }), false, 'connected', true), 'Upcoming appointment');
  assert.equal(callStatusTone('connected', false), 'slate', 'an ineligible appointment is never shown as green even mid-phase');
});

test('every real Agora connection phase for an eligible appointment maps to Phase 6\'s required wording', () => {
  const a = appointment({});
  assert.equal(callStatusText(a, true, 'idle', false), 'Ready to join');
  assert.equal(callStatusText(a, true, 'connecting', false), 'Connecting…');
  assert.equal(callStatusText(a, true, 'waiting-for-remote', false), 'Waiting for Patient…');
  assert.equal(callStatusText(a, true, 'connected', false), 'Waiting for Patient…', 'connected with no remote user yet must not falsely claim Connected');
  assert.equal(callStatusText(a, true, 'connected', true), 'Connected');
  assert.equal(callStatusText(a, true, 'reconnecting', false), 'Reconnecting…');
  assert.equal(callStatusText(a, true, 'failed', false), 'Connection failed');
  assert.equal(callStatusText(a, true, 'ended', false), 'Call ended');
});

test('status tone only turns green once a remote participant is actually present', () => {
  assert.equal(callStatusTone('connected', true), 'green');
  for (const phase of ['connecting', 'waiting-for-remote', 'reconnecting'] as const) assert.equal(callStatusTone(phase, true), 'amber');
  assert.equal(callStatusTone('failed', true), 'red');
  assert.equal(callStatusTone('idle', true), 'slate');
});

test('browser permission denial, device unavailability and network error codes each get a distinct, actionable message', () => {
  assert.match(classifyAgoraErrorCode('PERMISSION_DENIED') ?? '', /denied/i);
  assert.match(classifyAgoraErrorCode('DEVICE_NOT_FOUND') ?? '', /camera|microphone/i);
  assert.match(classifyAgoraErrorCode('NOT_READABLE') ?? '', /camera|microphone/i);
  assert.match(classifyAgoraErrorCode('NETWORK_ERROR') ?? '', /network/i);
  assert.match(classifyAgoraErrorCode('CAN_NOT_GET_GATEWAY_SERVER') ?? '', /unable to join/i);
  assert.match(classifyAgoraErrorCode('UID_CONFLICT') ?? '', /unable to join/i);
  assert.equal(classifyAgoraErrorCode('SOME_UNRECOGNIZED_CODE'), null, 'an unrecognized code must fall through to the caller\'s own generic/backend message, not a wrong specific one');
});
