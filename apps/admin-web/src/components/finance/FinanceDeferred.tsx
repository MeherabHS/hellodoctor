'use client';
import {AlertCircle,ClipboardCheck,CreditCard,Layers,ShieldCheck,Stethoscope,Wallet} from 'lucide-react';
import type {ReactNode} from 'react';
import {Card,Chip,StatTile} from '../ui/afya';

/**
 * Finance pages while FINANCIAL_MODULE_MODE=deferred (Campaigns & Finance
 * board, panels 3–5). Read-only and built from real values only: no simulated
 * ledger rows. The platform charge is always 20% (CLAUDE.md business rule 2),
 * and the mandatory note is shown on every earnings view.
 */
export const PLATFORM_CHARGE_NOTE = 'Total calculation is based including the platform charge 20%.';

function Banner({title,children}:{title:string;children:ReactNode}){
  return <div className="afya-finance-banner" role="status">
    <AlertCircle size={22} aria-hidden/>
    <div><strong>{title}</strong><span>{children}</span></div>
  </div>;
}

function InfoCard({icon:Icon,title,children}:{icon:typeof Wallet;title:ReactNode;children:ReactNode}){
  return <div className="afya-finance-info"><Icon size={20} aria-hidden/><div><strong>{title}</strong><span>{children}</span></div></div>;
}

export function PaymentsDeferred(){
  return <div className="afya-stack" style={{gap:16}}>
    <Banner title="Finance Module: Deferred (Read-Only)">Payments and escrow are not live yet. Consultations currently proceed without charge. Records, if any, stay visible below; no financial action can be taken.</Banner>
    <div className="afya-finance-grid">
      <InfoCard icon={CreditCard} title="Payments">In-app payment for consultations once the module is enabled.</InfoCard>
      <InfoCard icon={ShieldCheck} title="Escrow">A payment is held until the consultation is completed, then settled to the doctor.</InfoCard>
      <InfoCard icon={Layers} title={<>Status <Chip tone="warn">Deferred</Chip></>}>Enabled by deployment configuration in a future release.</InfoCard>
    </div>
  </div>;
}

export function SettlementsDeferred({consultations}:{consultations:number|null}){
  return <div className="afya-stack" style={{gap:16}}>
    <Banner title="Settlements & Payouts: Deferred (Read-Only)">Physician payouts are not live yet. Doctors are not paid through the platform while the module is deferred.</Banner>
    <div className="afya-stat-grid">
      <StatTile icon={Stethoscope} tone="green" value={consultations??'—'} label="Consultations booked"/>
      <StatTile icon={Wallet} tone="green" value="KES 0" label="Earnings to settle" note="No consultation is charged while deferred"/>
      <StatTile icon={ClipboardCheck} tone="amber" value={<Chip tone="warn">Deferred</Chip>} label="Next settlement"/>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>{PLATFORM_CHARGE_NOTE}</p>
  </div>;
}

export function SettingsDeferred(){
  return <div className="afya-stack" style={{gap:16}}>
    <Banner title="Finance Mode: Deferred (Read-Only)">Financial configuration cannot be changed until the finance module goes live. This page summarises the settings that will apply.</Banner>
    <div className="afya-two-col" style={{gridTemplateColumns:'1fr 1fr'}}>
      <Card title="Finance mode">
        <div style={{display:'flex',alignItems:'center',gap:12}}><span className="afya-stat-icon afya-tone-green" style={{width:46,height:46}}><Layers size={22} aria-hidden/></span><Chip tone="warn">Deferred</Chip></div>
        <p className="afya-stat-note" style={{margin:'10px 0 0'}}>Payments, escrow and settlements are switched off. Set by FINANCIAL_MODULE_MODE on the server.</p>
      </Card>
      <Card title="Module visibility (read-only)">
        <label className="afya-switch-row" style={{marginTop:0}}><span>Show payments in the apps</span><input type="checkbox" role="switch" checked={false} disabled readOnly/></label>
        <label className="afya-switch-row"><span>Show doctor wallet and escrow</span><input type="checkbox" role="switch" checked={false} disabled readOnly/></label>
      </Card>
      <Card title="Settlement rules">
        <dl className="afya-dl">
          <div><dt>Settlement frequency</dt><dd>Monthly statement</dd></div>
          <div><dt>Platform charge</dt><dd>20%</dd></div>
          <div><dt>Escrow holding period</dt><dd>Until the consultation is completed</dd></div>
          <div><dt>Payout method</dt><dd>Doctor&apos;s saved payout destination</dd></div>
        </dl>
        <p className="afya-stat-note" style={{margin:'8px 0 0'}}>{PLATFORM_CHARGE_NOTE}</p>
      </Card>
      <Card title="Audit & compliance">
        <ul className="afya-check-list">
          <li>Every payment, refund and payout action is written to the audit trail</li>
          <li>Payouts follow a finalised monthly statement</li>
          <li>Refunds need an Admin decision on a grievance</li>
          <li>Payment credentials are encrypted and never shown</li>
        </ul>
      </Card>
    </div>
  </div>;
}
