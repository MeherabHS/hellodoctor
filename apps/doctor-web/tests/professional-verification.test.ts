import test from 'node:test';
import assert from 'node:assert/strict';
import { isProfessionallyVerified, professionalVerificationLabel } from '../src/lib/professionalVerification.ts';
import type { Profile } from '../src/lib/contracts.ts';

test('professional review gates use explicit status and distinguish rejection', () => {
  for (const status of ['PENDING', 'VERIFIED', 'REJECTED'] as const) {
    const profile = { professional_verification_status: status, is_verified: status !== 'VERIFIED' };
    assert.equal(isProfessionallyVerified(profile), status === 'VERIFIED');
  }
  assert.equal(professionalVerificationLabel({ professional_verification_status: 'PENDING' }), 'Pending review');
  assert.equal(professionalVerificationLabel({ professional_verification_status: 'REJECTED' }), 'Professional registration rejected');
  assert.equal(isProfessionallyVerified(null), false);
  assert.equal(isProfessionallyVerified({ is_verified: true } as unknown as Profile), false);
});
