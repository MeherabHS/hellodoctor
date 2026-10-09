'use client';
import { useEffect, useRef } from 'react';

export function State({ loading, error, empty, retry }: { loading?: boolean; error?: string; empty?: string; retry?: () => void }) {
  if (!loading && !error && !empty) return null;
  return <div className="web-state" role={error ? 'alert' : 'status'} aria-live="polite">
    <div aria-hidden="true" style={{ marginBottom: 6 }}>{loading ? <span className="loading-spinner-ring web-state-spinner"/> : error ? <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><path d="M12 3 2 21h20L12 3Z"/><path d="M12 9v5M12 17v1"/></svg> : <span className="web-icon web-icon-calendar"/>}</div>
    <strong>{loading ? 'Loading…' : error || empty}</strong>
    {error && retry && <button className="slot-btn sec" onClick={retry}>Retry</button>}
  </div>;
}
export function Modal({ title, children, onClose, wide, bare }: { title: string; children: React.ReactNode; onClose: () => void; wide?: boolean; bare?: boolean }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = ref.current;
    root?.querySelector<HTMLElement>('button,input,textarea,select,[tabindex="0"]')?.focus();
    function handle(event: KeyboardEvent) {
      if (root !== document.querySelectorAll('[aria-modal="true"]')[document.querySelectorAll('[aria-modal="true"]').length - 1]) return;
      if (event.key === 'Escape') onClose();
      if (event.key !== 'Tab' || !root) return;
      const list = [...root.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),a[href],[tabindex="0"]')].filter(el => el.getClientRects().length);
      const first = list[0], last = list[list.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }
    document.addEventListener('keydown', handle);
    return () => { document.removeEventListener('keydown', handle); previous?.focus(); };
  }, [onClose]);
  return <div className="web-modal modal-backdrop" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
    <div ref={ref} className="bottom-sheet" role="dialog" aria-modal="true" aria-label={title} style={{ maxWidth: wide ? 860 : 560 }}>
      {!bare&&<><div className="sheet-pill-handle" />
      <div className="sheet-header-top"><h2 className="sheet-title">{title}</h2><button className="toast-dismiss-btn" aria-label="Close dialog" onClick={onClose}>✕</button></div></>}
      {children}
    </div>
  </div>;
}
