import test from 'node:test';
import assert from 'node:assert/strict';
import { getPendingDoctorVerifications, verifyDoctorProfessionalRegistration, rejectDoctorProfessionalRegistration } from '../src/lib/api.ts';

test('Doctor Verification uses the generic pending and review routes with the server state', async t => {
  const calls: Array<{ url: string; method: string; body: unknown }> = [];
  t.mock.method(globalThis, 'fetch', async (input: string, init: RequestInit) => {
    calls.push({ url: String(input), method: init.method || 'GET', body: init.body ? JSON.parse(String(init.body)) : null });
    return new Response(JSON.stringify({ success: true, data: input.endsWith('pending-verification')
      ? [{ id: 'synthetic', license_authority: 'KMPDC', license_country: 'KE', professional_verification_status: 'PENDING' }]
      : { professional_verification_status: input.endsWith('/verify') ? 'VERIFIED' : 'REJECTED' } }), { status: 200 });
  });
  const pending = await getPendingDoctorVerifications();
  assert.equal(pending[0].professional_verification_status, 'PENDING');
  assert.equal(pending[0].license_authority, 'KMPDC');
  assert.equal(pending[0].license_country, 'KE');
  assert.equal((await verifyDoctorProfessionalRegistration('synthetic', 'Manual review')).professional_verification_status, 'VERIFIED');
  assert.equal((await rejectDoctorProfessionalRegistration('synthetic', 'Incomplete submission')).professional_verification_status, 'REJECTED');
  assert.deepEqual(calls, [
    { url: '/api/proxy/admin/doctors/pending-verification', method: 'GET', body: null },
    { url: '/api/proxy/admin/doctors/synthetic/verify', method: 'POST', body: { note: 'Manual review' } },
    { url: '/api/proxy/admin/doctors/synthetic/reject', method: 'POST', body: { note: 'Incomplete submission' } },
  ]);
});

test('a conflicting review remains a failure for the Admin UI', async t => {
  t.mock.method(globalThis, 'fetch', async () => new Response('Only pending registrations can be reviewed.', { status: 409 }));
  await assert.rejects(verifyDoctorProfessionalRegistration('synthetic'), /409/);
  await assert.rejects(rejectDoctorProfessionalRegistration('synthetic'), /409/);
});
