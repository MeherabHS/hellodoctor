// Pure, dependency-free so it can be unit-tested directly (see
// tests/sanitize.test.ts) the same way client-incidents.ts is — server.ts
// itself can't be imported outside a Next.js request context because of its
// `next/headers` import.

// Protected words, matched component-wise (split on '_') so that a suffixed
// or pluralized key name — e.g. "access_tokens", "client_secrets" — is still
// caught. A naive boundary-delimited regex requiring a '_' or string-end on
// BOTH sides of the word misses these (h15-sanitize-redaction-regex-word-boundary-gap).
const SENSITIVE_KEY_WORDS = ['token', 'password', 'secret', 'object_key', 'object_bucket', 'encrypted', 'storage_path', 'api_key', 'authorization'];

function isSensitiveKey(key: string): boolean {
  const parts = key.toLowerCase().split('_');
  for (const word of SENSITIVE_KEY_WORDS) {
    const wordParts = word.split('_');
    for (let i = 0; i + wordParts.length <= parts.length; i++) {
      const slice = parts.slice(i, i + wordParts.length).join('_');
      if (slice === word || slice === `${word}s`) return true;
    }
  }
  return false;
}

export function sanitize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sanitize);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).filter(([key]) => !isSensitiveKey(key)).map(([key, item]) => [key, sanitize(item)]));
  return value;
}

// The ONE place a credential-shaped key must reach the browser: the Agora RTC
// join token minted by `POST consultations/:id/rtc-token`. The generic
// `sanitize()` above strips every `token` key, which silently removed this
// field, so the Agora SDK received `undefined` and rejected client.join()
// with INVALID_PARAMS. This is deliberately NOT a general exemption: it applies
// only to a successful response of that exact endpoint, and it rebuilds the
// payload from an allow-list of five fields with strict types, so nothing else
// from the upstream body (including any other token-like key) passes through.
const RTC_TOKEN_PATH = /^consultations\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/rtc-token$/i;

export function isRtcTokenMint(path: string, method: string): boolean {
  return method.toUpperCase() === 'POST' && RTC_TOKEN_PATH.test(path);
}

export function sanitizeRtcTokenResponse(value: unknown): unknown {
  const body = value as { success?: unknown; data?: Record<string, unknown> } | null;
  if (!body || body.success !== true || !body.data || typeof body.data !== 'object') return sanitize(value);
  const d = body.data;
  if (typeof d.app_id !== 'string' || typeof d.channel_name !== 'string' || typeof d.token !== 'string'
    || typeof d.uid !== 'number' || typeof d.expires_in !== 'number') return sanitize(value);
  return { success: true, data: { app_id: d.app_id, channel_name: d.channel_name, token: d.token, uid: d.uid, expires_in: d.expires_in } };
}
