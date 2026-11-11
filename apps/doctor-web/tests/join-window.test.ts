import test from 'node:test';
import assert from 'node:assert/strict';
import { describeJoinFailure, eatDateTime, formatCountdown, joinGate, lockedMessage, type JoinInfo } from '../src/lib/join-window.ts';
import { consultationAction, consultationActionLabel, consultationIntent, safeBackendPath } from '../src/lib/contracts.ts';
import { callFailureDiagnostic, describeCallFailure } from '../src/lib/call-failure.ts';
import { AGORA_CONNECT_SRC, PERMISSIONS_POLICY, buildCsp } from '../src/lib/csp.ts';
import { readFileSync } from 'node:fs';

// Appointment 12:00–12:10 UTC == 3:00–3:10 PM EAT. The server's policy (mirrored
// from the Rust evaluator's own unit tests): open 11:45:00Z, close 13:10:00Z.
const START = Date.parse('2026-11-11T12:00:00Z');
const END = Date.parse('2026-11-11T12:10:00Z');
const MIN = 60_000;
let uniq = 0;
// `serverNow` is when the *server* answered. Sub-millisecond digits make every case's
// server_time string unique (the per-response skew cache is keyed by it) without
// moving the instant, so boundary cases stay exact.
function join(serverNow: number, reason = 'OK'): JoinInfo {
  return { eligible: reason === 'OK', reason, join_open_at: new Date(START - 15 * MIN).toISOString(),
    join_close_at: new Date(END + 60 * MIN).toISOString(), server_time: new Date(serverNow).toISOString().replace('.000Z', `.000${String(uniq++).padStart(3, '0')}Z`) };
}
const gateAt = (serverNow: number) => { const j = join(serverNow); return joinGate(j, Date.parse(j.server_time)); };

test('16 minutes before start the room is locked with the exact countdown', () => {
  const g = gateAt(START - 16 * MIN);
  assert.equal(g.state, 'locked');
  assert.ok(Math.abs(g.msUntilOpen! - MIN) < 50);
});
test('exactly 15 minutes before start the room opens', () => {
  assert.equal(gateAt(START - 15 * MIN).state, 'open');
  assert.equal(gateAt(START - 15 * MIN - 1000).state, 'locked');
});
test('start, end and the post-end grace are all open; one second after the grace it closes', () => {
  for (const t of [START, END, END + 30 * MIN, END + 60 * MIN]) assert.equal(gateAt(t).state, 'open');
  assert.equal(gateAt(END + 60 * MIN + 1000).state, 'closed');
});
test('completed, cancelled and interrupted appointments are unavailable, never joinable', () => {
  for (const reason of ['INVALID_STATE', 'NOT_VIDEO', 'NEEDS_REVIEW']) {
    const j = join(START + 5 * MIN, reason);
    assert.equal(joinGate(j, Date.parse(j.server_time)).state, 'unavailable', reason);
  }
  const review = joinGate(join(START + 5 * MIN, 'NEEDS_REVIEW'), START + 5 * MIN);
  assert.match(lockedMessage(review)!, /needs review/);
  const dead = joinGate(join(START + 5 * MIN, 'INVALID_STATE'), START + 5 * MIN);
  assert.match(lockedMessage(dead)!, /can no longer be joined/);
});
test('a device clock hours off (any timezone) is corrected with the server time', () => {
  for (const driftHours of [3, 6, -6, 1]) {
    const serverNow = START - 16 * MIN;
    const j = join(serverNow);
    const deviceNow = serverNow + driftHours * 3_600_000; // wrong device clock at response time
    assert.equal(joinGate(j, deviceNow).state, 'locked', `drift ${driftHours}h`);
    // 61 s later on the (still wrong) device clock the window has genuinely opened.
    assert.equal(joinGate(j, deviceNow + 61_000).state, 'open', `drift ${driftHours}h`);
  }
});
test('no join info (older backend) does not invent a lock; the server still enforces at token time', () => {
  assert.equal(joinGate(undefined).state, 'open');
});

test('locked copy shows the EAT time, with the EAT date when it is not today', () => {
  const g = joinGate(join(START - 16 * MIN), START - 16 * MIN);
  // open_at 11:45Z == 2:45 PM EAT
  assert.equal(lockedMessage(g, Date.parse('2026-11-11T09:00:00Z')), 'Room opens at 2:45 PM EAT');
  assert.equal(lockedMessage(g, Date.parse('2026-11-09T09:00:00Z')), 'Room opens at 11 Nov, 2:45 PM EAT');
  assert.equal(eatDateTime('2026-11-11T20:24:00Z', Date.parse('2026-11-11T19:00:00Z')), '11:24 PM EAT');
});
test('countdown formatting', () => {
  assert.equal(formatCountdown(0), 'now');
  assert.equal(formatCountdown(65_000), '1m 05s');
  assert.equal(formatCountdown(3_720_000), '1h 02m');
});

test('start/join is offered only inside the window; intent (Rx, completion) is not gated', () => {
  const base = { modality: 'VIDEO', status: 'CONFIRMED', queue_actionable: true } as const;
  const early = { ...base, join: join(START - 16 * MIN) };
  const open = { ...base, join: join(START - 14 * MIN) };
  const nowOf = (a: { join: JoinInfo }) => Date.parse(a.join.server_time);
  assert.equal(consultationAction(early, nowOf(early)), null);
  assert.match(consultationActionLabel(early, nowOf(early)), /^Room opens at /);
  assert.equal(consultationAction(open, nowOf(open)), 'call');
  assert.equal(consultationActionLabel(open, nowOf(open)), 'Start video consultation');
  const closed = { ...base, join: join(END + 61 * MIN) };
  assert.equal(consultationAction(closed, nowOf(closed)), null);
  assert.equal(consultationActionLabel(closed, nowOf(closed)), 'Joining window closed');
  // the doctor can still reach prescription/completion actions after the window closes
  assert.equal(consultationIntent(closed, nowOf(closed)), 'call');
  // chat appointments never depend on the video window
  assert.equal(consultationAction({ modality: 'CHAT', status: 'CONFIRMED', queue_actionable: true, chat_locked: false, chat_expires_at: '2999-01-01T00:00:00Z' }), 'chat');
});

test('backend stable codes map to distinct, actionable messages', () => {
  const open = '2026-11-11T20:24:00Z';
  assert.match(describeJoinFailure({ status: 403, code: 'NOT_YET_OPEN' }, open, Date.parse('2026-11-11T19:00:00Z'))!, /not open yet.*11:24 PM EAT/);
  assert.match(describeJoinFailure({ status: 403, code: 'WINDOW_CLOSED' })!, /window .* has closed/);
  assert.match(describeJoinFailure({ status: 409, code: 'INVALID_STATE' })!, /not in an active state/);
  assert.match(describeJoinFailure({ status: 409, code: 'NEEDS_REVIEW' })!, /needs review/);
  assert.match(describeJoinFailure({ status: 409, code: 'RESOURCE_CONFLICT' })!, /already live/);
  assert.match(describeJoinFailure({ status: 401, code: 'UNAUTHORIZED' })!, /session has expired/);
  assert.match(describeJoinFailure({ status: 403, code: 'FORBIDDEN' })!, /not authorized/);
  assert.equal(describeJoinFailure({ status: 500, code: 'INTERNAL' }), null);
});

test('failures are classified by stage: token, SDK load, Agora join, devices, publish, timeout', () => {
  const api = { status: 403, code: 'NOT_YET_OPEN', message: 'not open' };
  assert.match(describeCallFailure('token', api), /not open yet/);
  assert.match(describeCallFailure('token', new TypeError('Failed to fetch')), /Network unavailable/);
  assert.match(describeCallFailure('sdk_load', new Error('chunk')), /video component could not be loaded/);
  assert.match(describeCallFailure('client_join', { code: 'JOIN_TIMEOUT' }), /agora\.io and sd-rtn\.com/);
  assert.match(describeCallFailure('client_join', { code: 'CAN_NOT_GET_GATEWAY_SERVER' }), /Unable to join/);
  assert.match(describeCallFailure('media_devices', { code: 'PERMISSION_DENIED' }), /Camera\/microphone access was denied/);
  assert.match(describeCallFailure('media_devices', new Error('x')), /Camera or microphone could not be started/);
  assert.match(describeCallFailure('publish', new Error('x')), /could not be sent/);
});
test('diagnostics are sanitized: stage and short code only — no tokens, messages or PHI', () => {
  const d = callFailureDiagnostic('client_join', { code: 'CAN_NOT_GET_GATEWAY_SERVER', name: 'AgoraRTCException', message: 'token=SECRET patient John Doe', token: 'SECRET' });
  assert.equal(d, 'stage=client_join code=CAN_NOT_GET_GATEWAY_SERVER name=AgoraRTCException');
  assert.ok(!/SECRET|John|token=/.test(d));
  assert.equal(callFailureDiagnostic('token', { status: 403, code: 'NOT_YET_OPEN', message: 'Rafiq Ahmed chest pain' }), 'stage=token http=403 code=NOT_YET_OPEN');
  assert.equal(callFailureDiagnostic('sdk_load', 'boom'), 'stage=sdk_load type=string');
});

test('production CSP lets the Agora SDK reach its gateways and Permissions-Policy allows the microphone', () => {
  const csp = buildCsp('abc', false);
  const connect = csp.split('; ').find(d => d.startsWith('connect-src'))!;
  for (const host of AGORA_CONNECT_SRC) assert.ok(connect.includes(host), host);
  assert.ok(connect.includes("'self'"));
  assert.ok(!csp.includes('unsafe-eval'), 'production must not allow unsafe-eval');
  assert.ok(!/connect-src[^;]*\*[^.]/.test(connect), 'no bare wildcard');
  assert.match(PERMISSIONS_POLICY, /microphone=\(self\)/);
  assert.match(PERMISSIONS_POLICY, /camera=\(self\)/);
  // next.config.js is plain CJS and cannot import the TS constant: assert it matches.
  assert.ok(readFileSync(new URL('../next.config.js', import.meta.url), 'utf8').includes(PERMISSIONS_POLICY));
});

test('the join-eligibility read is proxied; only GET', () => {
  const id = '11111111-1111-1111-1111-111111111111';
  assert.equal(safeBackendPath(`consultations/${id}/join-eligibility`, 'GET'), true);
  assert.equal(safeBackendPath(`consultations/${id}/join-eligibility`, 'POST'), false);
  assert.equal(safeBackendPath(`consultations/${id}/rtc-token`, 'POST'), true);
});
