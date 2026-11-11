'use client';
import { useState } from 'react';
import { api, errorText, useResource } from '../lib/client';
import { RecordDto, money } from '../lib/contracts';
import { State } from './Common';
import { date } from './Queue';

/** Shown in place of real earnings/payout UI whenever the server-authoritative
 * `GET /config/booking-policy` reports the financial module as deferred
 * (`FINANCIAL_MODULE_MODE=deferred`). Never shows fabricated earnings. */
export function DeferredFinancialNotice() {
  return <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Financial module not yet active</h3></div><div className="web-section-body"><p className="sheet-desc">Payment collection is not yet active for this launch. Consultations are proceeding without charge.</p><p className="sheet-desc">Earnings, settlements and payouts will appear here once the financial module is enabled.</p></div></section>;
}

export function Wallet({ statement, payout, receipt }: { statement:()=>void; payout:()=>void; receipt:(row:RecordDto)=>void }) {
  const wallet=useResource<RecordDto>('doctor/wallet/summary');
  const ledger=useResource<RecordDto[]>('doctor/wallet/ledger?limit=25&offset=0');
  const [pageBusy,setPageBusy]=useState(false);
  const [pageError,setPageError]=useState('');
  const [hasMore,setHasMore]=useState(true);
  async function more(){if(!ledger.data||pageBusy)return;setPageBusy(true);setPageError('');try{const next=await api<RecordDto[]>(`doctor/wallet/ledger?limit=25&offset=${ledger.data.length}`);ledger.setData([...ledger.data,...next]);setHasMore(next.length===25);}catch(e){setPageError(errorText(e));}finally{setPageBusy(false);}}
  const w=wallet.data;
  return <section className="dw-page-view active" id="webEarningsWorkspace">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>Doctor Earnings &amp; Wallet</h2><p>Transparent platform charges &amp; scheduled Finance disbursement &middot; KES</p></div><div className="web-action-row"><button className="dw-btn-outline" onClick={payout}>Payout Settings</button><button className="dw-btn-primary" onClick={statement}>Monthly Statement</button></div></header>
    <div className="web-wallet-grid">
      <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Settlement Breakdown</h3><span className="badge badge-green">Eligible earnings</span></div><div className="web-section-body"><State loading={wallet.loading} error={wallet.error} retry={()=>void wallet.refresh()}/>{[['Gross eligible earnings',w?.total_gross],['Platform charge',w?.withheld_fee],['Doctor earning (net)',w?.final_net],['Finalized unpaid',w?.finalized_unpaid],['Scheduled unpaid',w?.scheduled_unpaid],['Actual paid',w?.total_paid]].map(([label,value])=><div className="web-financial-row" key={String(label)}><span>{String(label)}</span><strong>{money(value)}</strong></div>)}<p className="sheet-desc">{String(w?.basis_note??'Amounts and payment states are supplied by the financial ledger.')}</p><p className="sheet-desc">Last disbursement: {date(w?.last_disbursement_at as string)} &middot; {money(w?.last_disbursement_amount)}</p><p className="sheet-desc">Finance handles scheduled disbursement. Eligible earnings do not confirm receipt of a payout.</p></div></section>
      <section className="dw-section-card"><div className="dw-section-header"><h3 className="dw-section-title">Consultation Ledger &amp; Platform Charge</h3></div><State loading={ledger.loading} error={ledger.error||pageError} empty={!ledger.loading&&!ledger.error&&ledger.data?.length===0?'No ledger entries':undefined} retry={()=>void ledger.refresh()}/>{!!ledger.data?.length&&<div className="dw-table-wrap"><table className="dw-data-table"><thead><tr><th>Consultation</th><th>Gross</th><th>Platform</th><th>Doctor earning</th><th>Financial state</th><th>Action</th></tr></thead><tbody>{ledger.data.map(row=><tr key={String(row.id)}><td><strong>{String(row.patient_display_name??row.transaction_number)}</strong><div>{date(String(row.created_at))}</div></td><td>{money(row.gross_amount)}</td><td>{money(row.platform_fee_amount)}</td><td>{money(row.net_amount)}</td><td>{String(row.payment_status).replaceAll('_',' ')}</td><td><button className="dw-btn-mini" onClick={()=>receipt(row)}>Fee breakdown</button></td></tr>)}</tbody></table></div>}{hasMore&&(ledger.data?.length??0)>=25&&<div className="web-section-body"><button className="dw-btn-outline" disabled={pageBusy} onClick={()=>void more()}>{pageBusy?'Loading...':'Load more entries'}</button></div>}</section>
    </div>
  </section>;
}

export function PayoutSettings() {
  const destinations=useResource<RecordDto[]>('doctor/wallet/payout-methods');
  return <><p className="sheet-desc">Finance handles scheduled disbursements. Provider authorization is required to connect a real M-Pesa or Kenyan bank destination.</p><State loading={destinations.loading} error={destinations.error} retry={()=>void destinations.refresh()}/>{destinations.data?.map(d=><div key={String(d.id)} className="dw-pt-card"><div className="web-action-row"><strong>{d.method_type==='MPESA'?'M-Pesa':d.method_type==='BANK_TRANSFER'?'Bank transfer':'Provider destination'}</strong><span className="badge badge-green">{d.is_primary?'Primary':''} {String(d.binding_status)}</span></div><p>{String(d.masked_destination)}</p><p className="web-gap">{d.provider_code==='DEV_SIMULATED'?'Development simulation · No real provider payment':String(d.verification_status)} · {d.payout_capable?'Payout capable':'Not ready'}</p></div>)}{destinations.data?.length===0&&<State empty="No payout destination connected"/>}<div className="doc-earnings-calc-note">Real provider authorization is awaiting external integration. No account number, card details, PIN or OTP are collected here.</div></>;
}
