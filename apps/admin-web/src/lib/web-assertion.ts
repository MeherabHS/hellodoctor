import { createHash, createHmac, randomBytes } from 'node:crypto';

// The Rust API requires this assertion for Admin password entry in deployed
// environments, so calling the origin API cannot skip the BFF challenge gate.
export function webAuthAssertion(body: string, secret: string | undefined, environment: string | undefined,
  timestamp = Math.floor(Date.now() / 1000), nonce = randomBytes(16).toString('hex')): Record<string, string> {
  if (environment === 'development' || environment === 'test') return {};
  if (!['production', 'staging'].includes(environment ?? '') || !secret || secret.length < 32 || /\s/.test(secret)) {
    throw new Error('Secure authentication configuration required.');
  }
  const digest = createHash('sha256').update(body).digest('hex');
  const message = `v1\n${timestamp}\n${nonce}\nPOST\n/api/v1/auth/admin/login\n${digest}`;
  const mac = createHmac('sha256', secret).update(message).digest('hex');
  return { 'x-hellodoctor-web-proof': `v1.${timestamp}.${nonce}.${mac}` };
}
