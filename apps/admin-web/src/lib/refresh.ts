import { createClient, type RedisClientType } from 'redis';
import { createHash, randomUUID } from 'crypto';

type Tokens = { access_token: string; refresh_token: string };

// What coordinatedRefresh() actually decided for this request:
//   'rotated' — we (or our same-process leader) obtained fresh tokens; caller
//               should set the new cookies and retry.
//   'failed'  — the backend authoritatively rejected the refresh (invalid,
//               expired, or already-reused token); caller should clear the
//               session, same as before this change.
//   'raced'   — a DIFFERENT admin-web process is/was rotating this exact
//               refresh token right now. We deliberately never attempted our
//               own rotate() call in this case (see below), so the caller
//               must NOT clear the session: the browser will carry the
//               winner's freshly-rotated cookie on its next request.
export type RefreshResult =
  | { outcome: 'rotated'; tokens: Tokens }
  | { outcome: 'failed' }
  | { outcome: 'raced' };

const LOCK_TTL_MS = 8000; // Bounds how long a crashed/hung leader can block followers.
const LOCK_POLL_INTERVAL_MS = 150;
const LOCK_WAIT_BUDGET_MS = 3000; // Bounded wait for a follower; always returns, never deadlocks.

function fingerprint(refreshToken: string): string {
  // Coordination keys are derived from a one-way hash of the refresh token —
  // the raw token is never written to Redis, as a key, value, or elsewhere.
  return createHash('sha256').update(refreshToken).digest('hex');
}

// `redisClientOverride` is a testability seam ONLY: production code (the
// `coordinatedRefresh` singleton below) never passes it and gets the normal
// lazily-created, self-managed, process-lifetime connection. Tests that need
// deterministic control over a real Redis connection's lifecycle (so they can
// `disconnect()` it themselves and let the test process exit cleanly) create
// their own client and pass it in instead of relying on this module's
// internal lazy singleton.
export function refreshCoordinator(graceMs = 5000, redisClientOverride?: RedisClientType | null) {
  // Same-process dedupe. Concurrent dashboard reads on ONE admin-web process
  // carry the same HttpOnly refresh cookie; rotate it once, rather than
  // treating normal parallel reads as token-reuse attempts. Unchanged from
  // before this fix — this fast path never touches Redis.
  const rotations = new Map<string, Promise<RefreshResult>>();
  const instanceId = randomUUID();

  // Each coordinator instance (one per admin-web process, in production) owns
  // its own lazily-created Redis connection attempt, evaluated from
  // process.env.REDIS_URL the first time it's actually needed. `undefined` =
  // not yet attempted; `null` = REDIS_URL unset or connection failed, meaning
  // cross-process coordination is unavailable and we fall back to
  // same-process-only dedupe (this module's original behavior, which is
  // correct for the single-instance deployment this repo currently
  // evidences). Any Redis failure here fails OPEN to that fallback rather
  // than breaking login.
  let redisClient: RedisClientType | null | undefined;
  let redisConnectPromise: Promise<RedisClientType | null> | null = null;

  async function getRedis(): Promise<RedisClientType | null> {
    if (redisClientOverride !== undefined) return redisClientOverride;
    if (redisClient !== undefined) return redisClient;
    const url = process.env.REDIS_URL;
    if (!url) {
      redisClient = null;
      return null;
    }
    if (redisConnectPromise) return redisConnectPromise;
    redisConnectPromise = (async () => {
      try {
        const client = createClient({ url, socket: { connectTimeout: 2000, reconnectStrategy: false } });
        client.on('error', () => {
          // Swallow async client errors so they never crash the process; the
          // next call to getRedis() will simply fail open.
        });
        await client.connect();
        redisClient = client as RedisClientType;
      } catch {
        redisClient = null;
      }
      return redisClient;
    })();
    return redisConnectPromise;
  }

  return async (refresh: string, rotate: () => Promise<Tokens | null>): Promise<RefreshResult> => {
    const existing = rotations.get(refresh);
    if (existing) return existing;

    // IMPORTANT: the promise below must be created, and `rotations.set()`
    // called, with NO `await` in between on this synchronous call path (the
    // async work — including getRedis() — happens *inside* the IIFE, not
    // before it's created). That keeps same-process dedupe exactly as
    // synchronous as the original process-local-only implementation: several
    // callers racing the same `refresh` value in the same tick must all see
    // the map entry the first one reserves, with no await-induced gap where
    // two callers could both observe an empty map and each start their own
    // rotate()/lock attempt.
    const p = (async (): Promise<RefreshResult> => {
      const redis = await getRedis();
      if (!redis) {
        // No cross-process coordination available/configured — original
        // single-process behavior, verbatim.
        const tokens = await rotate();
        return tokens ? { outcome: 'rotated' as const, tokens } : { outcome: 'failed' as const };
      }

      // Cross-process mutex, keyed by a hash of the token. Only the instance
      // that wins this lock is allowed to ever call rotate() for this token —
      // this is the actual fix: a losing instance must NEVER independently
      // attempt the same refresh token, because the backend's single-use
      // rotation guard would see that as reuse and revoke the whole session
      // family out from under an admin who was doing nothing wrong.
      const lockKey = `admin-web:refresh-lock:${fingerprint(refresh)}`;
      const acquired = await redis.set(lockKey, instanceId, { NX: true, PX: LOCK_TTL_MS }).catch(() => null);

      if (acquired === 'OK') {
        try {
          const tokens = await rotate();
          return tokens ? { outcome: 'rotated' as const, tokens } : { outcome: 'failed' as const };
        } finally {
          // Best-effort compare-and-delete release; the PX TTL above is the
          // real safety net if this fails or the process dies mid-rotation,
          // so a crashed leader can never deadlock other instances for more
          // than LOCK_TTL_MS.
          await redis
            .eval(
              'if redis.call("GET", KEYS[1]) == ARGV[1] then return redis.call("DEL", KEYS[1]) else return 0 end',
              { keys: [lockKey], arguments: [instanceId] },
            )
            .catch(() => {});
        }
      }

      // We lost the race: some other process already holds the rotation
      // lock for this exact token. Wait briefly (bounded — this loop always
      // exits within LOCK_WAIT_BUDGET_MS, so it can never deadlock the
      // request) for the leader to finish, then report 'raced' regardless
      // of outcome. The caller must leave the session cookies untouched in
      // this case: the leader's response already set a fresh cookie on ITS
      // request, and the browser will carry it on the next call.
      const deadline = Date.now() + LOCK_WAIT_BUDGET_MS;
      while (Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, LOCK_POLL_INTERVAL_MS));
        const stillLocked = await redis.exists(lockKey).catch(() => 0);
        if (!stillLocked) break;
      }
      return { outcome: 'raced' as const };
    })();

    rotations.set(refresh, p);
    void p.finally(() => {
      setTimeout(() => rotations.delete(refresh), graceMs).unref();
    });
    return p;
  };
}

export const coordinatedRefresh = refreshCoordinator();
