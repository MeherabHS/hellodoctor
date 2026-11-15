// The backend admits at most two `/api/v1/admin/*` requests at a time
// (services/backend/src/admission.rs: the "Expensive" lane, `n.min(2)`) and
// answers 503 beyond that. Admin Web pages load several things at once
// (lists, counts, thumbnails, background polling), so its own calls queue
// here instead of racing into that ceiling. Reads rejected with 503 are
// retried a couple of times with a short back-off; writes are never retried.
// Plain .mjs (like inflight.mjs) so Node's test runner can load it directly.

export const ADMIN_SLOTS = 2;
let active = 0;
/** @type {(() => void)[]} */
const waiting = [];

/**
 * Runs `task` once one of the admin request slots is free.
 * @template T
 * @param {() => Promise<T>} task
 * @returns {Promise<T>}
 */
export async function withAdminSlot(task) {
  if (active >= ADMIN_SLOTS) await new Promise(resolve => waiting.push(() => resolve(undefined)));
  active++;
  try {
    return await task();
  } finally {
    active--;
    waiting.shift()?.();
  }
}

/** @param {number} ms */
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

/**
 * Fetch through the admin slots. A GET/HEAD answered 503 (admission full) is
 * retried up to `retries` times; anything else is returned as is.
 * @param {string} input
 * @param {RequestInit} [init]
 * @param {number} [retries]
 * @param {number} [backoffMs]
 * @returns {Promise<Response>}
 */
export async function adminFetch(input, init = {}, retries = 2, backoffMs = 400) {
  const method = (init.method ?? 'GET').toUpperCase();
  const idempotent = method === 'GET' || method === 'HEAD';
  for (let attempt = 0; ; attempt++) {
    const response = await withAdminSlot(() => fetch(input, init));
    if (response.status !== 503 || !idempotent || attempt >= retries) return response;
    await pause(backoffMs * (attempt + 1));
  }
}

/** For tests: current queue state. */
export const adminQueueState = () => ({ active, waiting: waiting.length });
