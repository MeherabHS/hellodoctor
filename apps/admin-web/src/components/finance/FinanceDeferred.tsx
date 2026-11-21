'use client';
import { tr } from '@/lib/i18n';
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
    <Banner title={tr('ui.financeModuleDeferredReadOnly')}>{tr('ui.paymentsAndEscrowAreNotLive')}</Banner>
    <div className="afya-finance-grid">
      <InfoCard icon={CreditCard} title={tr('ui.payments')}>{tr('ui.inAppPaymentForConsultationsOnce')}</InfoCard>
      <InfoCard icon={ShieldCheck} title={tr('ui.escrow2')}>{tr('ui.aPaymentIsHeldUntilThe')}</InfoCard>
      <InfoCard icon={Layers} title={<>{tr('ui.status')} <Chip tone="warn">{tr('ui.deferred')}</Chip></>}>{tr('ui.enabledByDeploymentConfigurationInA')}</InfoCard>
    </div>
  </div>;
}

export function SettlementsDeferred({consultations}:{consultations:number|null}){
  return <div className="afya-stack" style={{gap:16}}>
    <Banner title={tr('ui.settlementsPayoutsDeferredReadOnly')}>{tr('ui.physicianPayoutsAreNotLiveYet')}</Banner>
    <div className="afya-stat-grid">
      <StatTile icon={Stethoscope} tone="green" value={consultations??'—'} label={tr('ui.consultationsBooked')}/>
      <StatTile icon={Wallet} tone="green" value="KES 0" label={tr('ui.earningsToSettle')} note={tr('ui4.noConsultationIsChargedWhileDeferred')}/>
      <StatTile icon={ClipboardCheck} tone="amber" value={<Chip tone="warn">{tr('ui.deferred')}</Chip>} label={tr('ui.nextSettlement')}/>
    </div>
    <p className="afya-stat-note" style={{margin:0}}>{PLATFORM_CHARGE_NOTE}</p>
  </div>;
}

export function SettingsDeferred(){
  return <div className="afya-stack" style={{gap:16}}>
    <Banner title={tr('ui.financeModeDeferredReadOnly')}>{tr('ui.financialConfigurationCannotBeChangedUnt')}</Banner>
    <div className="afya-two-col" style={{gridTemplateColumns:'1fr 1fr'}}>
      <Card title={tr('ui.financeMode')}>
        <div style={{display:'flex',alignItems:'center',gap:12}}><span className="afya-stat-icon afya-tone-green" style={{width:46,height:46}}><Layers size={22} aria-hidden/></span><Chip tone="warn">{tr('ui.deferred')}</Chip></div>
        <p className="afya-stat-note" style={{margin:'10px 0 0'}}>{tr('ui.paymentsEscrowAndSettlementsAreSwitched')}</p>
      </Card>
      <Card title={tr('ui.moduleVisibilityReadOnly')}>
        <label className="afya-switch-row" style={{marginTop:0}}><span>{tr('ui.showPaymentsInTheApps')}</span><input type="checkbox" role="switch" checked={false} disabled readOnly/></label>
        <label className="afya-switch-row"><span>{tr('ui.showDoctorWalletAndEscrow')}</span><input type="checkbox" role="switch" checked={false} disabled readOnly/></label>
      </Card>
      <Card title={tr('ui.settlementRules')}>
        <dl className="afya-dl">
          <div><dt>{tr('ui.settlementFrequency')}</dt><dd>{tr('ui.monthlyStatement')}</dd></div>
          <div><dt>{tr('ui.platformCharge')}</dt><dd>20%</dd></div>
          <div><dt>{tr('ui.escrowHoldingPeriod')}</dt><dd>{tr('ui.untilTheConsultationIsCompleted')}</dd></div>
          <div><dt>{tr('ui.payoutMethod')}</dt><dd>{tr('ui.doctorSSavedPayoutDestination')}</dd></div>
        </dl>
        <p className="afya-stat-note" style={{margin:'8px 0 0'}}>{PLATFORM_CHARGE_NOTE}</p>
      </Card>
      <Card title={tr('ui.auditCompliance')}>
        <ul className="afya-check-list">
          <li>{tr('ui.everyPaymentRefundAndPayoutAction')}</li>
          <li>{tr('ui.payoutsFollowAFinalisedMonthlyStatement')}</li>
          <li>{tr('ui.refundsNeedAnAdminDecisionOn')}</li>
          <li>{tr('ui.paymentCredentialsAreEncryptedAndNever')}</li>
        </ul>
      </Card>
    </div>
  </div>;
}
