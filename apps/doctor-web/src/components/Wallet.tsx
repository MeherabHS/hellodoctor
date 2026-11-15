'use client';
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { RecordDto, money } from '../lib/contracts';
import { Badge, PageHeader, State } from './Common';
import { Icon } from './Icons';
import { PLATFORM_NOTE } from './Dashboard';
import { date } from './Queue';

/** Shown instead of earnings whenever `GET /config/booking-policy` reports the financial
 * module as deferred. Never shows fabricated earnings. */
export function DeferredFinancialNotice() {
  return <section className="am-card"><div className="am-card-body"><State icon="wallet" empty="Earnings are not active yet" hint="Payment collection has not started for this launch, so consultations are currently free for patients. Your earnings, settlements and payouts will appear here once it is switched on."/></div></section>;
}

export function Wallet({ statement, payout, receipt }: { statement: () => void; payout: () => void; receipt: (row: RecordDto) => void }) {
  const wallet = useResource<RecordDto>('doctor/wallet/summary');
  const ledger = useResource<RecordDto[]>('doctor/wallet/ledger?limit=25&offset=0');
  const [pageBusy, setPageBusy] = useState(false); const [pageError, setPageError] = useState(''); const [hasMore, setHasMore] = useState(true);
  async function more() { if (!ledger.data || pageBusy) return; setPageBusy(true); setPageError(''); try { const next = await api<RecordDto[]>(`doctor/wallet/ledger?limit=25&offset=${ledger.data.length}`); ledger.setData([...ledger.data, ...next]); setHasMore(next.length === 25); } catch (e) { setPageError(errorText(e)); } finally { setPageBusy(false); } }
  const w = wallet.data;
  return <section id="webEarningsWorkspace" className="am-stack" style={{ gap: 22 }}>
    <PageHeader title="Earnings" subtitle="Transparent platform charge and scheduled Finance disbursements, in KES.">
      <button className="am-btn ghost" onClick={payout}><Icon name="settings" size={16}/>Payout settings</button>
      <button className="am-btn primary" onClick={statement}><Icon name="file" size={16}/>Monthly statement</button>
    </PageHeader>
    <div className="am-grid main-side" style={{ alignItems: 'start' }}>
      <section className="am-card">
        <div className="am-card-head"><div><h2><Icon name="wallet"/>Settlement breakdown</h2><p>Lifetime eligible earnings</p></div><Badge tone="green">KES</Badge></div>
        <div className="am-card-body am-stack">
          <State loading={wallet.loading} error={wallet.error} retry={() => void wallet.refresh()}/>
          {w && <>
            <div className="am-grid three" style={{ gap: 12 }}>
              {[['1. Gross earnings', money(w.total_gross), 'var(--am-ink)'], ['2. Platform charge (20%)', `− ${money(w.withheld_fee)}`, 'var(--am-red-fg)'], ['3. Net take-home', money(w.final_net), 'var(--am-green-deep)']].map(([label, value, color]) =>
                <div key={label} className="am-card pad" style={{ boxShadow: 'none', background: 'var(--am-head)' }}><span className="am-faint">{label}</span><div style={{ fontSize: 22, fontWeight: 800, color, marginTop: 4 }}>{value}</div></div>)}
            </div>
            <p className="am-strong" style={{ fontSize: 13 }}>{PLATFORM_NOTE}</p>
            <dl className="am-kv">
              <dt>Finalized, not yet paid</dt><dd>{money(w.finalized_unpaid)}</dd>
              <dt>Scheduled, not yet paid</dt><dd>{money(w.scheduled_unpaid)}</dd>
              <dt>Paid to you</dt><dd>{money(w.total_paid)}</dd>
              <dt>Last disbursement</dt><dd>{w.last_disbursement_at ? `${date(w.last_disbursement_at as string)} · ${money(w.last_disbursement_amount)}` : '—'}</dd>
            </dl>
            <p className="am-faint">Amounts and payment states come from the financial ledger. Finance handles disbursement; an eligible earning does not confirm a payout.</p>
          </>}
        </div>
      </section>
      <div className="am-callout"><Icon name="info"/><span>Every consultation fee is split the same way: the gross fee, then the 20% platform charge, then your net take-home. Open any ledger line to see its breakdown.</span></div>
    </div>
    <section className="am-card">
      <div className="am-card-head"><h2><Icon name="file"/>Consultation ledger</h2></div>
      <State loading={ledger.loading} error={ledger.error || pageError} empty={!ledger.loading && !ledger.error && ledger.data?.length === 0 ? 'No ledger entries yet' : undefined} retry={() => void ledger.refresh()} icon="wallet"/>
      {!!ledger.data?.length && <div className="am-table-wrap"><table className="am-table">
        <thead><tr><th>Consultation</th><th className="num">Gross</th><th className="num">Platform (20%)</th><th className="num">Net</th><th>State</th><th aria-label="Breakdown"/></tr></thead>
        <tbody>{ledger.data.map(row => <tr key={String(row.id)}>
          <td><strong className="am-strong">{String(row.patient_display_name ?? row.transaction_number)}</strong><div className="am-faint">{date(String(row.created_at))}</div></td>
          <td className="num">{money(row.gross_amount)}</td><td className="num" style={{ color: 'var(--am-red-fg)' }}>− {money(row.platform_fee_amount)}</td><td className="num" style={{ color: 'var(--am-green-deep)', fontWeight: 700 }}>{money(row.net_amount)}</td>
          <td><Badge tone={String(row.payment_status) === 'DISBURSED' ? 'green' : 'slate'}>{String(row.payment_status).replaceAll('_', ' ').toLowerCase()}</Badge></td>
          <td><button className="am-btn ghost sm" onClick={() => receipt(row)}>Breakdown</button></td>
        </tr>)}</tbody>
      </table></div>}
      {hasMore && (ledger.data?.length ?? 0) >= 25 && <div className="am-card-body am-row" style={{ justifyContent: 'center' }}><button className="am-btn ghost" disabled={pageBusy} onClick={() => void more()}>{pageBusy ? 'Loading…' : 'Load more entries'}</button></div>}
    </section>
  </section>;
}

export function PayoutSettings() {
  const destinations = useResource<RecordDto[]>('doctor/wallet/payout-methods');
  return <div className="am-stack">
    <p className="am-muted">Finance sends scheduled disbursements to your connected destination. Connecting a real M-Pesa or Kenyan bank destination needs provider authorization.</p>
    <State compact loading={destinations.loading} error={destinations.error} retry={() => void destinations.refresh()}/>
    {destinations.data?.map(d => <div key={String(d.id)} className="am-file"><span className="am-file-icon"><Icon name="wallet" size={20}/></span><div className="am-grow"><strong className="am-strong" style={{ display: 'block' }}>{d.method_type === 'MPESA' ? 'M-Pesa' : d.method_type === 'BANK_TRANSFER' ? 'Bank transfer' : 'Provider destination'} · {String(d.masked_destination)}</strong><span className="am-faint">{d.provider_code === 'DEV_SIMULATED' ? 'Development simulation · no real payment' : String(d.verification_status)} · {d.payout_capable ? 'Ready for payouts' : 'Not ready'}</span></div>{d.is_primary ? <Badge tone="green">Primary</Badge> : null}<Badge>{String(d.binding_status)}</Badge></div>)}
    {destinations.data?.length === 0 && <State compact icon="wallet" empty="No payout destination connected"/>}
    <div className="am-callout blue"><Icon name="lock"/><span>No account numbers, card details, PINs or OTPs are collected here. Provider authorization is awaiting external integration.</span></div>
  </div>;
}
