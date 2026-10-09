import { useCallback, useEffect, useRef, useState } from 'react';
import { AdminSlot, getAdminSlots } from './api';

// Schedule reads have their own lifecycle: unrelated dashboard work must never
// delay a slot mutation, conceal a failed refresh, or overwrite its result.
export function useSlotSync(onUpdate: (rows: AdminSlot[]) => void) {
  const callback = useRef(onUpdate);
  callback.current = onUpdate;
  const epoch = useRef(0);
  const [sync, setSync] = useState({ loading: true, error: '', updatedAt: '' });
  const refresh = useCallback(async () => {
    const job = ++epoch.current;
    setSync(previous => ({ ...previous, loading: true }));
    try {
      const rows = await getAdminSlots();
      if (job !== epoch.current) return;
      callback.current(rows);
      setSync({ loading: false, error: '', updatedAt: new Date().toISOString() });
    } catch {
      if (job === epoch.current) setSync(previous => ({ ...previous, loading: false, error: 'Schedule refresh failed. Displayed slots may be out of date.' }));
    }
  }, []);
  useEffect(() => {
    void refresh();
    let pending = false;
    const update = async () => {
      if (pending || document.visibilityState !== 'visible' || !navigator.onLine) return;
      pending = true;
      try { await refresh(); } finally { pending = false; }
    };
    const tick = () => { void update(); };
    const timer = window.setInterval(tick, 15000);
    window.addEventListener('focus', tick);
    window.addEventListener('online', tick);
    document.addEventListener('visibilitychange', tick);
    return () => {
      epoch.current++;
      window.clearInterval(timer);
      window.removeEventListener('focus', tick);
      window.removeEventListener('online', tick);
      document.removeEventListener('visibilitychange', tick);
    };
  }, [refresh]);
  return { ...sync, refresh };
}
