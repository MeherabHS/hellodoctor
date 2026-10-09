// Refresh only a mounted, visible schedule. The backend remains authoritative;
// cross-portal changes cannot be observed through local mutation callbacks.
export function startScheduleSync(refresh: () => Promise<void>, browser: {
  window: Pick<Window, 'addEventListener' | 'removeEventListener'>;
  document: Pick<Document, 'addEventListener' | 'removeEventListener' | 'visibilityState'>;
  online: () => boolean;
  every: (callback: () => void, milliseconds: number) => unknown;
  cancel: (timer: unknown) => void;
}) {
  let active = true;
  let pending = false;
  const update = async () => {
    if (!active || pending || browser.document.visibilityState !== 'visible' || !browser.online()) return;
    pending = true;
    try { await refresh(); } finally { pending = false; }
  };
  const onChange = () => { void update(); };
  const timer = browser.every(onChange, 15000);
  browser.window.addEventListener('focus', onChange);
  browser.window.addEventListener('online', onChange);
  browser.document.addEventListener('visibilitychange', onChange);
  return () => {
    active = false;
    browser.cancel(timer);
    browser.window.removeEventListener('focus', onChange);
    browser.window.removeEventListener('online', onChange);
    browser.document.removeEventListener('visibilitychange', onChange);
  };
}
