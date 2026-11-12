import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { isRtcTokenMint, sanitize, sanitizeRtcTokenResponse } from '../src/lib/sanitize.ts';
import { validateJoinArgs } from '../src/lib/join-args.ts';

// Regression for `stage=client_join code=INVALID_PARAMS`. Root cause: the Next BFF
// proxy ran `sanitize()` over every upstream body, and `sanitize()` strips every key
// named `token` — including the Agora RTC token. The browser therefore received
// { app_id, channel_name, uid, expires_in } with NO token, so client.join() was
// called with token === undefined and the SDK rejected it with INVALID_PARAMS
// ("Invalid token: undefined"). The SDK itself, the CSP and the other three
// arguments were fine.

// The exact shape services/backend/src/api/v1/consultation_handlers.rs returns
// (app_id/channel_name/uid/expires_in/token; uid is a u32, channel is rtc_<32 hex>).
const APPOINTMENT = '123e4567-e89b-12d3-a456-426614174000';
const PATH = `consultations/${APPOINTMENT}/rtc-token`;
const backend = () => ({
  success: true,
  data: {
    channel_name: 'rtc_0123456789abcdef0123456789abcdef',
    token: '007' + 'eJxjYGBg'.repeat(30),
    uid: 3735928559,
    app_id: 'a1b2c3d4e5f60718293a4b5c6d7e8f90',
    expires_in: 3600,
  },
});

test('the generic sanitizer strips the RTC token (documents the root cause)', () => {
  const out = sanitize(backend()) as { data: Record<string, unknown> };
  assert.equal('token' in out.data, false);
  assert.deepEqual(Object.keys(out.data).sort(), ['app_id', 'channel_name', 'expires_in', 'uid']);
  const report = validateJoinArgs(out.data);
  assert.equal(report.ok, false);
  assert.equal(report.field, 'token');
});

test('the RTC mint response keeps exactly the five join fields, with their types', () => {
  const out = sanitizeRtcTokenResponse(backend()) as { success: boolean; data: Record<string, unknown> };
  assert.equal(out.success, true);
  assert.deepEqual(Object.keys(out.data).sort(), ['app_id', 'channel_name', 'expires_in', 'token', 'uid']);
  assert.equal(typeof out.data.token, 'string');
  assert.equal(typeof out.data.uid, 'number');
  assert.equal(validateJoinArgs(out.data).ok, true);
});

test('the exemption is exact: only POST consultations/<uuid>/rtc-token', () => {
  assert.equal(isRtcTokenMint(PATH, 'POST'), true);
  for (const [p, m] of [[PATH, 'GET'], [`${PATH}/x`, 'POST'], ['consultations/not-a-uuid/rtc-token', 'POST'],
    [`consultations/${APPOINTMENT}/join-eligibility`, 'POST'], ['auth/refresh', 'POST'], [`x/${PATH}`, 'POST']] as const) {
    assert.equal(isRtcTokenMint(p, m), false, `${m} ${p}`);
  }
});

test('the exemption never leaks extra or mistyped fields', () => {
  const body = backend();
  (body.data as Record<string, unknown>).refresh_token = 'LEAK';
  (body.data as Record<string, unknown>).app_certificate = 'LEAK';
  const out = sanitizeRtcTokenResponse(body) as { data: Record<string, unknown> };
  assert.equal(JSON.stringify(out).includes('LEAK'), false);
  // A malformed body falls back to the generic (redacting) sanitizer.
  const bad = { success: true, data: { ...backend().data, uid: '3735928559' } };
  assert.equal('token' in (sanitizeRtcTokenResponse(bad) as { data: Record<string, unknown> }).data, false);
  const failure = { success: false, error: { code: 'NOT_YET_OPEN', token: 'LEAK' } };
  assert.equal(JSON.stringify(sanitizeRtcTokenResponse(failure)).includes('LEAK'), false);
});

test('join-argument diagnostics expose types and lengths only, never values', () => {
  const secret = backend().data;
  const ok = validateJoinArgs(secret);
  assert.equal(ok.ok, true);
  for (const v of [secret.token, secret.app_id, secret.channel_name, String(secret.uid)]) assert.equal(ok.diagnostic.includes(v), false);
  assert.match(ok.diagnostic, /token:string\/\d+\/ok/);
  assert.match(ok.diagnostic, /uid:number\/-\/ok/);
  for (const [field, patch] of [['token', { token: undefined }], ['token', { token: '' }], ['uid', { uid: 1.5 }], ['uid', { uid: -1 }],
    ['uid', { uid: '5' }], ['channel_name', { channel_name: 'x'.repeat(65) }], ['channel_name', { channel_name: '' }], ['app_id', { app_id: null }]] as const) {
    const r = validateJoinArgs({ ...secret, ...patch });
    assert.equal(r.ok, false, JSON.stringify(patch));
    assert.equal(r.field, field);
  }
});

// The production-shaped response, through the proxy sanitizer, the adapter's
// `json.data` unwrap and the pre-flight, into the REAL Agora SDK's client.join().
function realClient() {
  const require = createRequire(import.meta.url);
  const code = readFileSync(require.resolve('agora-rtc-sdk-ng'), 'utf8');
  const noop = () => undefined;
  class Stub {}
  const ctx: Record<string, unknown> = {
    console: { log: noop, warn: noop, error: noop, info: noop, debug: noop }, URL, TextEncoder, TextDecoder, Blob, performance,
    setTimeout: noop, clearTimeout: noop, setInterval: noop, clearInterval: noop, addEventListener: noop, removeEventListener: noop,
    HTMLVideoElement: Stub, HTMLMediaElement: Stub, HTMLElement: Stub, MediaStream: Stub, MediaStreamTrack: Stub, RTCPeerConnection: Stub,
    WebSocket: Stub, Worker: Stub, AudioContext: Stub, fetch: () => new Promise(() => undefined), isSecureContext: true,
    location: { href: 'https://doctor.test/', protocol: 'https:', hostname: 'doctor.test', origin: 'https://doctor.test' },
    navigator: { userAgent: 'Mozilla/5.0 Chrome/120', platform: 'Win32', onLine: true, mediaDevices: { enumerateDevices: async () => [], addEventListener: noop } },
    document: { createElement: () => ({}), addEventListener: noop, body: {} },
  };
  ctx.self = ctx; ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(code, ctx, { timeout: 15000 });
  return (ctx.AgoraRTC as { createClient(c: object): { join(...a: unknown[]): Promise<unknown> } }).createClient({ mode: 'rtc', codec: 'vp8' });
}
const PENDING = Symbol('pending');
const outcome = (p: Promise<unknown>) => Promise.race([p.then(() => 'joined', (e: { code?: string }) => e.code ?? 'error'), new Promise<symbol>(r => setTimeout(() => r(PENDING), 50))]);

test('real SDK: the token-less (old proxy) payload is rejected with INVALID_PARAMS', async () => {
  const data = (sanitize(backend()) as { data: Record<string, unknown> }).data as { app_id: string; channel_name: string; token: string; uid: number };
  assert.equal(await outcome(realClient().join(data.app_id, data.channel_name, data.token, data.uid)), 'INVALID_PARAMS');
});

test('real SDK: the production-shaped payload passes the SDK\'s argument validation', async () => {
  const data = (sanitizeRtcTokenResponse(backend()) as { data: { app_id: string; channel_name: string; token: string; uid: number } }).data;
  assert.equal(validateJoinArgs(data).ok, true);
  // Valid arguments do not produce INVALID_PARAMS; with no network here the join stays pending.
  const result = await outcome(realClient().join(data.app_id, data.channel_name, data.token, data.uid));
  assert.notEqual(result, 'INVALID_PARAMS');
});
