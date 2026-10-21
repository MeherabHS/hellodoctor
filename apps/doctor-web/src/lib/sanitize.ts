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
