'use client';
import {useEffect, useState} from 'react';
import {DatabaseBackup} from 'lucide-react';
import {request} from '../../lib/api';
import {Card, Chip, type ChipTone} from '../ui/afya';

/**
 * Nightly database dump status (deploy/single-vm/nightly-dump), from
 * `GET /admin/system/backup-status`. Admin-only: backup timing is useful to
 * an attacker, so it is never on a public page.
 */
export type BackupState = 'HEALTHY' | 'FAILED' | 'OVERDUE' | 'NEVER_RUN' | 'NOT_CONFIGURED';
export interface BackupStatus {
  state: BackupState;
  checked_at: string;
  overdue_after_hours: number;
  last_run: {at: string; result: 'ok' | 'failed'; message?: string} | null;
  last_success: {at: string; file: string; size_bytes: number; dumps_kept: number; total_bytes: number; keep: number} | null;
  last_restore_test: {at: string; result: 'ok' | 'failed'; file: string; migrations?: number} | null;
}

const STATE: Record<BackupState, {tone: ChipTone; label: string; text: string}> = {
  HEALTHY: {tone: 'success', label: 'Healthy', text: 'Last night\'s backup finished and was verified.'},
  FAILED: {tone: 'danger', label: 'Failed', text: 'The last backup run failed. See the reason below and fix it before tonight\'s run.'},
  OVERDUE: {tone: 'warn', label: 'Overdue', text: 'No successful backup in the expected window. The nightly timer may have stopped.'},
  NEVER_RUN: {tone: 'warn', label: 'Never run', text: 'The status folder is connected, but no backup has run yet.'},
  NOT_CONFIGURED: {tone: 'neutral', label: 'Not set up', text: 'Nightly backups are not installed on this server, or their status folder is not mounted.'},
};

const when = (iso?: string) => iso ? new Date(iso).toLocaleString('en-KE', {timeZone: 'Africa/Nairobi', day: 'numeric', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit'}) + ' EAT' : '—';
export function size(bytes?: number): string {
  if (!bytes || bytes < 1) return '—';
  const units = ['B', 'KB', 'MB', 'GB'];
  let v = bytes, i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

export function BackupStatusCard() {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    let active = true;
    const load = () => request<BackupStatus>('/admin/system/backup-status')
      .then(s => { if (active) { setStatus(s); setError(false); } })
      .catch(() => { if (active) setError(true); });
    void load();
    const id = setInterval(() => void load(), 5 * 60_000);
    return () => { active = false; clearInterval(id); };
  }, []);

  const state = status ? STATE[status.state] : null;
  return <Card title={<span style={{display: 'inline-flex', alignItems: 'center', gap: 8}}><DatabaseBackup size={18} aria-hidden/>Database backups</span>}
    subtitle="Nightly at 03:00 EAT · newest copies kept on the server"
    actions={state && <Chip tone={state.tone}>{state.label}</Chip>}>
    {error && <p className="afya-stat-note" role="alert" style={{margin: 0}}>The backup status could not be loaded.</p>}
    {!status && !error && <p className="afya-stat-note" style={{margin: 0}}>Loading…</p>}
    {status && state && <>
      <p style={{margin: '0 0 12px'}}>{state.text}</p>
      {status.state === 'FAILED' && status.last_run?.message && <p className="afya-login-error" role="alert" style={{marginTop: 0}}>{status.last_run.message}</p>}
      {status.state !== 'NOT_CONFIGURED' && <dl className="afya-timeline-summary">
        <div><dt>Last successful backup</dt><dd>{when(status.last_success?.at)}</dd></div>
        <div><dt>Size</dt><dd>{size(status.last_success?.size_bytes)}</dd></div>
        <div><dt>Copies kept</dt><dd>{status.last_success ? `${status.last_success.dumps_kept} of ${status.last_success.keep} · ${size(status.last_success.total_bytes)}` : '—'}</dd></div>
        <div><dt>Last run</dt><dd>{status.last_run ? `${when(status.last_run.at)} · ${status.last_run.result === 'ok' ? 'ok' : 'failed'}` : '—'}</dd></div>
        <div><dt>Last test restore</dt><dd>{status.last_restore_test ? `${when(status.last_restore_test.at)} · ${status.last_restore_test.result === 'ok' ? 'restored and checked' : 'failed'}` : 'Not yet (runs on Sundays)'}</dd></div>
        <div><dt>Overdue after</dt><dd>{status.overdue_after_hours} hours</dd></div>
      </dl>}
    </>}
  </Card>;
}
