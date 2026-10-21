// Server-side only. Never import this module into a client component.
export type ChallengeConfig = { environment?: string; origin?: string; secret?: string };
export type ChallengeResult = { ok: true } | { ok: false; status: 403 | 503 };
export function challengeRequired(environment?: string) {
  return environment !== 'development' && environment !== 'test';
}

export class AuthBodyError extends Error {
  readonly status: 400 | 408 | 413 | 503;
  constructor(status: 400 | 408 | 413 | 503) { super('Invalid sign-in form.'); this.status = status; }
}
export function createAuthBodyReader(maxConcurrent = 16) {
  let active = 0;
  return async (request: Request): Promise<Record<string, unknown>> => {
    if (active >= maxConcurrent) throw new AuthBodyError(503);
    active++;
    try { return await readBoundedAuthBody(request); }
    finally { active--; }
  };
}
export const readAuthBody = createAuthBodyReader();
async function readBoundedAuthBody(request: Request): Promise<Record<string, unknown>> {
  if (Number(request.headers.get('content-length')) > 16384) throw new AuthBodyError(413);
  const reader = request.body?.getReader();
  if (!reader) throw new AuthBodyError(400);
  let expired = false;
  const timeout = setTimeout(() => { expired = true; void reader.cancel().catch(() => {}); }, 5000);
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (expired) throw new AuthBodyError(408);
      if (done) break;
      length += value.byteLength;
      if (length > 16384) { void reader.cancel().catch(() => {}); throw new AuthBodyError(413); }
      chunks.push(value);
    }
    let data;
    try { data = JSON.parse(Buffer.concat(chunks).toString('utf8')); }
    catch { throw new AuthBodyError(400); }
    if (!data || typeof data !== 'object' || Array.isArray(data)) throw new AuthBodyError(400);
    return data;
  } finally { clearTimeout(timeout); reader.releaseLock(); }
}

// This bounds Siteverify requests per BFF process, before any upstream auth work.
// The backend's global/account/IP budgets remain authoritative across replicas.
export function createChallengeVerifier(maxConcurrent = 8, clock: () => number = () => performance.now()) {
  let inFlight = 0;
  let tokens = 60;
  let lastRefill = clock();
  return async function verify(token: unknown, action: string, config: ChallengeConfig,
    send: typeof fetch = fetch, now = Date.now()): Promise<ChallengeResult> {
    if (!challengeRequired(config.environment)) return { ok: true };
    let origin: URL;
    try { origin = new URL(config.origin ?? ''); } catch { return { ok: false, status: 503 }; }
    if (!['production', 'staging'].includes(config.environment ?? '') || origin.protocol !== 'https:' ||
      origin.origin !== config.origin || !config.secret || /^[123]x0{10}/.test(config.secret)) {
      return { ok: false, status: 503 };
    }
    if (typeof token !== 'string' || token.length === 0 || token.length > 2048) return { ok: false, status: 403 };
    if (inFlight >= maxConcurrent) return { ok: false, status: 503 };
    const current = clock();
    tokens = Math.min(60, tokens + Math.max(0, current - lastRefill) * 12 / 1000);
    lastRefill = current;
    if (tokens < 1) return { ok: false, status: 503 };
    tokens--;
    inFlight++;
    try {
      const response = await send('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ secret: config.secret, response: token }),
        cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(4000),
      });
      if (!response.ok) return { ok: false, status: 503 };
      const data = await response.json();
      const timestamp = Date.parse(data?.challenge_ts);
      if (data?.success !== true || data.hostname !== origin.hostname || data.action !== action ||
        !Number.isFinite(timestamp) || timestamp < now - 300000 || timestamp > now + 30000) {
        return { ok: false, status: 403 };
      }
      // No success cache: every submitted token is consumed through Siteverify.
      return { ok: true };
    } catch { return { ok: false, status: 503 }; }
    finally { inFlight--; }
  };
}
export const verifyChallenge = createChallengeVerifier();
