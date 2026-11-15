'use client';
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { Profile, RecordDto, Statement as StatementDto, money, payoutLabel, statementLabel } from '../lib/contracts';
import { Badge, State } from './Common';
import { PLATFORM_NOTE } from './Dashboard';
import { date } from './Queue';

export function Statement({ profile }: { profile: Profile }) {
  const [month, setMonth] = useState(new Date().toISOString().slice(0, 7));
  const result = useResource<StatementDto>(`doctor/wallet/statements?month=${month}&limit=50&offset=0`);
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const s = result.data;
  async function more() { if (!s || s.next_offset === null || busy) return; setBusy(true); setError(''); try { const next = await api<StatementDto>(`doctor/wallet/statements?month=${month}&limit=50&offset=${s.next_offset}`); result.setData({ ...next, entries: [...s.entries, ...next.entries] }); } catch (e) { setError(errorText(e)); } finally { setBusy(false); } }
  const destination = s?.payout_destination as RecordDto | null | undefined;
  return <div className="am-stack">
    <div className="am-row between">
      <div><strong className="am-strong">{profile.full_name}</strong><p className="am-faint">Earnings and platform charge statement</p></div>
      <div className="am-row"><label className="am-field" style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}><span className="am-faint">Month</span><input className="am-input" aria-label="Statement month" type="month" value={month} min="2000-01" max={new Date().toISOString().slice(0, 7)} style={{ minHeight: 36, padding: '4px 10px' }} onChange={e => { if (e.target.value) setMonth(e.target.value); }}/></label>{s && <Badge tone={s.status === 'FINALIZED' ? 'green' : 'amber'}>{s.status.replaceAll('_', ' ').toLowerCase()}</Badge>}</div>
    </div>
    <State loading={result.loading} error={result.error || error} retry={() => void result.refresh()}/>
    {s && <>
      <dl className="am-kv">
        <dt>1. Gross earnings</dt><dd>{money(s.gross_earnings)}</dd>
        <dt>2. Platform charge (20%)</dt><dd style={{ color: 'var(--am-red-fg)' }}>− {money(s.platform_charge)}</dd>
        <dt>Adjustments</dt><dd>{money(s.adjustments)}</dd>
        <dt>3. {statementLabel(s)}</dt><dd style={{ color: 'var(--am-green-deep)' }}>{money(s.payable)}</dd>
      </dl>
      <p className="am-strong" style={{ fontSize: 13 }}>{s.basis_note || PLATFORM_NOTE}</p>
      <p className="am-faint">Disbursement: {payoutLabel(s.payout_status, destination?.provider_code)} · Destination: {String(destination?.masked_destination ?? 'Not configured')}</p>
      <div className="am-table-wrap" style={{ border: '1px solid var(--am-line)', borderRadius: 14 }}><table className="am-table">
        <thead><tr><th>Consultation</th><th className="num">Gross</th><th className="num">Platform</th><th className="num">Net</th></tr></thead>
        <tbody>{s.entries.map((row, index) => <tr key={String(row.consultation_reference) + index}><td><strong className="am-strong">{String(row.consultation_reference)}</strong><div className="am-faint">{date(row.date as string)} · {String(row.financial_status).replaceAll('_', ' ').toLowerCase()}</div></td><td className="num">{money(row.gross_fee)}</td><td className="num" style={{ color: 'var(--am-red-fg)' }}>{money(row.platform_charge)}</td><td className="num" style={{ color: 'var(--am-green-deep)', fontWeight: 700 }}>{money(row.doctor_earning)}</td></tr>)}</tbody>
      </table></div>
      {s.entries.length === 0 && <State compact icon="wallet" empty="No eligible consultations this month"/>}
      {s.next_offset !== null && <button className="am-btn ghost" disabled={busy} onClick={() => void more()}>{busy ? 'Loading…' : 'Load more entries'}</button>}
    </>}
  </div>;
}
