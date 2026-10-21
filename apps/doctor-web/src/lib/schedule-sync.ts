// Refresh only a mounted, visible schedule. The backend remains authoritative;
// cross-portal changes cannot be observed through local mutation callbacks.
//
// On repeated failure the poll backs off (doubling, capped, with bounded
// jitter) instead of retrying at a fixed cadence forever, so many clients
// recovering from the same outage don't retry in lockstep. A caller that
// already tracks its own error state (most `refresh` callbacks here catch
// their own exceptions rather than rethrow) can opt in via `options.hasError`;
// callers that omit it keep the original fixed-interval behavior unchanged.
export function startScheduleSync(refresh: () => Promise<void>, browser: {
  window: Pick<Window, 'addEventListener' | 'removeEventListener'>;
  document: Pick<Document, 'addEventListener' | 'removeEventListener' | 'visibilityState'>;
  online: () => boolean;
  every: (callback: () => void, milliseconds: number) => unknown;
  cancel: (timer: unknown) => void;
}, milliseconds = 15000, options?: {
  hasError?: () => boolean;
  maxMilliseconds?: number;
  jitterRatio?: number;
  random?: () => number;
  initial?: boolean;
}) {
  let active = true;
  let pending = false;
  let consecutiveFailures = 0;
  let timer: unknown = null;
  let timerVersion = 0;
  let recovering = false;
  const maxMs = options?.maxMilliseconds ?? milliseconds * 8;
  const jitterRatio = options?.jitterRatio ?? 0.2;
  const random = options?.random ?? Math.random;

  const currentDelay = () => {
    if (consecutiveFailures === 0) return milliseconds;
    const shift = Math.min(consecutiveFailures, 6);
    const backoff = Math.min(milliseconds * 2 ** shift, maxMs);
    return backoff + random() * backoff * jitterRatio;
  };

  const update = async () => {
    if (!active || pending || browser.document.visibilityState !== 'visible' || !browser.online()) return;
    pending = true;
    try {
      await refresh();
      consecutiveFailures = options?.hasError?.() ? consecutiveFailures + 1 : 0;
    } catch (e) {
      consecutiveFailures += 1;
      throw e;
    } finally {
      pending = false;
    }
  };

  const armNext = (ms: number) => {
    if (!active) return;
    if (timer !== null) browser.cancel(timer);
    const version = ++timerVersion;
    timer = browser.every(() => { if (version === timerVersion) onTick(); }, ms);
  };

  const onTick = () => {
    if (!active || pending) return;
    const version = ++timerVersion;
    recovering = false;
    if (timer !== null) { browser.cancel(timer); timer = null; }
    update()
      .catch(() => { /* consecutiveFailures already recorded above */ })
      .finally(() => { if (version === timerVersion && timer === null) armNext(currentDelay()); });
  };

  const onChange = () => {
    if (!active || pending || recovering || browser.document.visibilityState !== 'visible' || !browser.online()) return;
    // A shared online/focus event must not reset outage history or synchronize
    // all clients. Bursts share this one timer; manual refresh stays immediate.
    recovering = true;
    armNext(250 + Math.floor(random() * 1751));
  };

  if (options?.initial) onTick(); else armNext(milliseconds);
  browser.window.addEventListener('focus', onChange);
  browser.window.addEventListener('online', onChange);
  browser.document.addEventListener('visibilitychange', onChange);
  return () => {
    active = false;
    timerVersion++;
    if (timer !== null) browser.cancel(timer);
    browser.window.removeEventListener('focus', onChange);
    browser.window.removeEventListener('online', onChange);
    browser.document.removeEventListener('visibilitychange', onChange);
  };
}
