import { useCallback, useEffect, useRef, useState } from 'react';
import { AdminSlot, getAdminSlots } from './api';
import { startScheduleSync } from './polling';

const BASE_INTERVAL_MS = 15000;

// Schedule reads have their own lifecycle: unrelated dashboard work must never
// delay a slot mutation, conceal a failed refresh, or overwrite its result.
//
// On repeated failure the poll backs off (doubling, capped, with bounded
// jitter) instead of retrying at a fixed 15s cadence forever, so recovery
// from a shared backend outage doesn't produce a synchronized retry storm
// across every open admin session. A successful refresh resets the cadence.
export function useSlotSync(onUpdate: (rows: AdminSlot[]) => void, enabled=true) {
  const callback = useRef(onUpdate);
  useEffect(() => { callback.current = onUpdate; }, [onUpdate]);
  const epoch = useRef(0);
  const [sync, setSync] = useState({ loading: true, error: '', updatedAt: '' });
  const hasError = useRef(false);
  const refresh = useCallback(async () => {
    const job = ++epoch.current;
    setSync(previous => ({ ...previous, loading: true }));
    try {
      const rows = await getAdminSlots();
      if (job !== epoch.current) return;
      callback.current(rows);
      hasError.current = false;
      setSync({ loading: false, error: '', updatedAt: new Date().toISOString() });
    } catch {
      if (job === epoch.current) {
        hasError.current = true;
        setSync(previous => ({ ...previous, loading: false, error: 'Schedule refresh failed. Displayed slots may be out of date.' }));
      }
    }
  }, []);
  useEffect(() => {
    if(!enabled)return;
    const activeEpoch = epoch;
    const stop = startScheduleSync(refresh, {
      window, document, online: () => navigator.onLine,
      every: (callback, ms) => window.setTimeout(callback, ms),
      cancel: timer => window.clearTimeout(timer as number),
    }, BASE_INTERVAL_MS, { hasError: () => hasError.current, initial: true });
    return () => {
      activeEpoch.current++;
      stop();
    };
  }, [refresh,enabled]);
  return { ...sync, refresh };
}
