const crypto = require('node:crypto');
const http = require('node:http');
const cases = [];
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const makeKey = () => crypto.generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
const key = makeKey();
const other = makeKey();
function proof(binding, method, path, options = {}) {
  const pair = options.key || key;
  const publicJwk = pair.publicKey.export({ format: 'jwk' });
  const header = { typ: 'hd-device+jwt', alg: 'ES256', jwk: publicJwk };
  const body = { htm: options.method || method, htu: options.path || path,
    iat: Math.floor(Date.now() / 1000) + (options.offset || 0),
    jti: crypto.randomUUID(), binding: crypto.createHash('sha256').update(options.binding || binding).digest('base64url') };
  const input = Buffer.from(JSON.stringify(header)).toString('base64url') + '.' + Buffer.from(JSON.stringify(body)).toString('base64url');
  return input + '.' + crypto.sign('sha256', Buffer.from(input), { key: pair.privateKey, dsaEncoding: 'ieee-p1363' }).toString('base64url');
}
async function request(caseId, method, path, body, token, signed, expected, required = false) {
  await sleep(220);
  const data = body ? Buffer.from(JSON.stringify(body)) : null;
  const headers = { 'X-Forwarded-For': '203.0.113.20', 'X-Forwarded-Proto': 'https' };
  if (data) { headers['Content-Type'] = 'application/json'; headers['Content-Length'] = data.length; }
  if (token) headers.Authorization = 'Bearer ' + token;
  if (signed) headers['X-Device-Proof'] = signed;
  if (required) headers['X-Device-Proof-Required'] = '1';
  const response = await new Promise((resolve, reject) => {
    const req = http.request({ hostname: '127.0.0.1', port: 18080, method, path, headers, timeout: 2000 }, (res) => {
      const chunks = []; let size = 0;
      res.on('data', (chunk) => { size += chunk.length; if (size > 1024 * 1024) req.destroy(new Error('bounded-response')); else chunks.push(chunk); });
      res.on('end', () => resolve({ status: res.statusCode, id: res.headers['x-request-id'], body: Buffer.concat(chunks) }));
    });
    req.on('timeout', () => req.destroy(new Error('fixture-timeout')));
    req.on('error', reject);
    req.end(data);
  });
  cases.push({ case_id: caseId, status: response.status, expected_status: expected, request_id: response.id, result: response.status === expected ? 'PASS' : 'FAIL' });
  if (response.status !== expected) throw new Error(caseId);
  return JSON.parse(response.body.toString('utf8'));
}
async function bootstrap(phone, suffix) {
  const legal = await request('legal-' + suffix, 'GET', '/api/v1/legal/documents', null, null, null, 200);
  const consent = legal.data.find((doc) => doc.document_type === 'PATIENT_DATA_SHARING_CONSENT');
  if (!consent) throw new Error('consent-not-configured');
  const otp = await request('otp-' + suffix, 'POST', '/api/v1/auth/register-otp', { phone_number: phone }, null, null, 200);
  const path = '/api/v1/auth/verify-otp-and-login';
  const body = { phone_number: phone, otp_code: otp.data.debug_otp_code,
    accepted_documents: [{ type: 'PATIENT_DATA_SHARING_CONSENT', version: consent.version }] };
  const login = await request('bound-establishment-' + suffix, 'POST', path, body, null, proof(phone, 'POST', path), 200, true);
  cases.push({ case_id: 'device-bound-flag-' + suffix, result: login.data.device_bound === true ? 'PASS' : 'FAIL' });
  if (login.data.device_bound !== true) throw new Error('device-bound-required');
  return login.data;
}
(async () => {
  let failure = null;
  try {
    const session = await bootstrap('+254799000098', 'rotation');
    const path = '/api/v1/patients/me';
    const access = session.access_token;
    await request('access-without-proof', 'GET', path, null, access, null, 401);
    await request('access-valid-proof', 'GET', path, null, access, proof(access, 'GET', path), 200);
    await request('access-wrong-key', 'GET', path, null, access, proof(access, 'GET', path, { key: other }), 401);
    await request('access-wrong-method', 'GET', path, null, access, proof(access, 'GET', path, { method: 'POST' }), 401);
    await request('access-wrong-path', 'GET', path, null, access, proof(access, 'GET', path, { path: path + '/changed' }), 401);
    await request('access-stale-proof', 'GET', path, null, access, proof(access, 'GET', path, { offset: -120 }), 401);
    await request('access-future-proof', 'GET', path, null, access, proof(access, 'GET', path, { offset: 120 }), 401);
    await request('access-wrong-token-binding', 'GET', path, null, access, proof(access, 'GET', path, { binding: access + '-changed' }), 401);
    const replay = proof(access, 'GET', path);
    await request('access-replay-first', 'GET', path, null, access, replay, 200);
    await request('access-replay-second', 'GET', path, null, access, replay, 401);
    const refreshPath = '/api/v1/auth/refresh';
    await request('stolen-refresh-without-proof', 'POST', refreshPath, { refresh_token: session.refresh_token }, null, null, 401);
    await request('legitimate-access-after-stolen-refresh', 'GET', path, null, access, proof(access, 'GET', path), 200);
    const rotated = await request('refresh-valid-proof', 'POST', refreshPath, { refresh_token: session.refresh_token }, null, proof(session.refresh_token, 'POST', refreshPath), 200);
    await request('rotated-access-valid', 'GET', path, null, rotated.data.access_token, proof(rotated.data.access_token, 'GET', path), 200);
    await request('used-refresh-with-valid-key', 'POST', refreshPath, { refresh_token: session.refresh_token }, null, proof(session.refresh_token, 'POST', refreshPath), 401);
    await request('reused-family-access-revoked', 'GET', path, null, rotated.data.access_token, proof(rotated.data.access_token, 'GET', path), 401);
    const logoutSession = await bootstrap('+254799000097', 'logout');
    const logoutPath = '/api/v1/auth/logout';
    await request('logout-bound-session', 'POST', logoutPath, { refresh_token: logoutSession.refresh_token }, null, proof(logoutSession.refresh_token, 'POST', logoutPath), 200);
    await request('logout-access-revoked', 'GET', path, null, logoutSession.access_token, proof(logoutSession.access_token, 'GET', path), 401);
    await request('logout-refresh-revoked', 'POST', refreshPath, { refresh_token: logoutSession.refresh_token }, null, proof(logoutSession.refresh_token, 'POST', refreshPath), 401);
  } catch (error) { failure = error.message; }
  console.log(JSON.stringify({ scope: 'Synthetic P-256 client to actual Rust HTTP backend; not Android Keystore or physical-device acceptance',
    cases, passed: cases.filter((row) => row.result === 'PASS').length, failed: cases.filter((row) => row.result === 'FAIL').length, failure,
    private_keys_saved: false, credentials_logged: false, verdict: failure ? 'FAIL' : 'PASS' }));
  process.exitCode = failure ? 1 : 0;
})();
