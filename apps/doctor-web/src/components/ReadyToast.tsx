'use client';
import { useState } from 'react';
import { Appointment } from '../lib/contracts';
import { Icon } from './Icons';
import { time } from './Queue';

/** "Ready for consultation" pop-up: shown once per appointment when the server's own
 * join window (via consultationAction) makes it startable. Dismissals live in memory only. */
const dismissed = new Set<string>();
export function ReadyToast({ appointment: a, onStart, onDetails }: { appointment: Appointment; onStart: () => void; onDetails: () => void }) {
  const [, rerender] = useState(0);
  if (dismissed.has(a.id)) return null;
  const close = () => { dismissed.add(a.id); rerender(n => n + 1); };
  return <aside className="am-toast" role="status" aria-live="polite">
    <div className="am-row between" style={{ flexWrap: 'nowrap', alignItems: 'flex-start' }}>
      <span className="am-stat-icon" style={{ width: 44, height: 44 }}><Icon name={a.modality === 'CHAT' ? 'chat' : 'video'} size={22}/></span>
      <div className="am-grow" style={{ padding: '0 12px' }}><strong className="am-strong" style={{ display: 'block' }}>Consultation ready</strong><p className="am-muted" style={{ fontSize: 13.5 }}>{a.patient_display_name} · {time(a.start_time)}. You can start now.</p></div>
      <button className="am-close" aria-label="Dismiss" onClick={close}><Icon name="close" size={16}/></button>
    </div>
    <div className="am-row" style={{ marginTop: 14 }}><button className="am-btn primary am-grow" onClick={() => { close(); onStart(); }}>Start consultation</button><button className="am-btn ghost" onClick={() => { close(); onDetails(); }}>Details</button></div>
  </aside>;
}
