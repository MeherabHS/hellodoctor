'use client';
import { tr } from '@/lib/i18n';
import { apiErrorMessage, parseApiError } from '@/lib/i18n/apiErrors';
import {useEffect,useState} from 'react';
import {KeyRound,MailPlus,RefreshCw,ShieldCheck,UserCheck,Users,XCircle} from 'lucide-react';
import {getTeam,inviteAdmin,isPlainEmail,resendInvitation,revokeInvitation,type CreatedInvitation,type InvitationStatus,type TeamAdmin,type TeamInvitation} from '../../lib/teamApi';
import {Card,Chip,EmptyState,PageHead,StatTile,eatDate,type ChipTone} from '../ui/afya';

/**
 * Admin Team: invite other people to become Admins by email (product owner,
 * 2026-11-14). The invitee gets a single-use link valid for 5 minutes and sets
 * their own phone number and password; nobody else ever sees their password.
 * Any Admin may invite, resend or revoke (single-Admin rule).
 */
const STATUS:Record<InvitationStatus,{tone:ChipTone;label:string}>={PENDING:{tone:'warn',get label(){return tr('ui.pending')}},ACCEPTED:{tone:'success',get label(){return tr('ui.accepted')}},REVOKED:{tone:'neutral',get label(){return tr('ui.revoked')}},EXPIRED:{tone:'danger',get label(){return tr('ui.expired')}}};

/** The backend's user-facing message from an "API Error [status]: {json}" error, never raw detail. */
function errorText(e:unknown):string{
  const fallback=tr('ui2.theRequestCouldNotBeCompleted');
  return parseApiError(e).status?apiErrorMessage(e,fallback):fallback;
}

export function AdminTeamPage(){
  const [admins,setAdmins]=useState<TeamAdmin[]>([]);const [invitations,setInvitations]=useState<TeamInvitation[]>([]);
  const [loading,setLoading]=useState(true);const [loadError,setLoadError]=useState('');const [revision,setRevision]=useState(0);
  const [email,setEmail]=useState('');const [password,setPassword]=useState('');const [busy,setBusy]=useState<string|null>(null);const [error,setError]=useState('');const [result,setResult]=useState<CreatedInvitation|null>(null);
  useEffect(()=>{let active=true;
    getTeam().then(t=>{if(active){setAdmins(t.admins);setInvitations(t.invitations);setLoadError('');}})
      .catch(()=>{if(active)setLoadError(tr('ui2.theTeamCouldNotBeLoaded'));})
      .finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;};},[revision]);
  const pending=invitations.filter(i=>i.status==='PENDING');
  const valid=isPlainEmail(email);

  async function run(key:string,action:()=>Promise<unknown>):Promise<boolean>{
    setBusy(key);setError('');setResult(null);
    try{const r=await action();if(r&&typeof r==='object'&&'invitation' in r)setResult(r as CreatedInvitation);setRevision(v=>v+1);return true;}
    catch(e){setError(errorText(e));return false;}
    finally{setBusy(null);setPassword('');}
  }
  const hasPassword=password.length>0&&password.length<=128;
  async function invite(e:React.FormEvent){e.preventDefault();if(!valid||!hasPassword||busy)return;const pw=password;if(await run('invite',()=>inviteAdmin(email.trim(),pw)))setEmail('');}

  return <div className="afya-stack">
    <PageHead title={tr('ui.adminTeam')} subtitle={tr('ui4.invitePeopleToBecomeAdminsThey')}/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="green" value={loading?'—':admins.length} label={tr('ui.admins')}/>
      <StatTile icon={MailPlus} tone="amber" value={loading?'—':pending.length} label={tr('ui.pendingInvitations')}/>
      <StatTile icon={ShieldCheck} tone="blue" value="5 min" label={tr('ui.invitationLifetime')} note={tr('ui4.singleUseRevocable')}/>
    </div>

    <Card title={tr('ui.inviteAnAdmin')} subtitle={tr('ui4.onlyInviteFullAccess')}>
      <form onSubmit={invite} className="afya-filter-bar" style={{marginBottom:0}} noValidate>
        <label className="afya-search" style={{flex:'1 1 320px'}}>
          <MailPlus size={16} color="var(--afya-faint)" aria-hidden/>
          <input type="email" aria-label={tr('ui.emailAddressToInvite')} placeholder={tr('ui.nameExampleOrg')} autoComplete="off" maxLength={254} value={email} onChange={e=>{setEmail(e.target.value);setError('');}}/>
        </label>
        <label className="afya-search" style={{flex:'0 1 260px'}}>
          <KeyRound size={16} color="var(--afya-faint)" aria-hidden/>
          <input type="password" aria-label={tr('ui.yourCurrentPassword')} placeholder={tr('ui.yourCurrentPassword')} autoComplete="current-password" maxLength={128} value={password} onChange={e=>{setPassword(e.target.value);setError('');}}/>
        </label>
        <button type="submit" className="afya-btn-primary" disabled={!valid||!hasPassword||busy!==null}><MailPlus size={16} aria-hidden/>{busy==='invite'?tr('ui2.sending'):tr('ui2.sendInvitation')}</button>
      </form>
      {!hasPassword&&pending.length>0&&<p className="afya-stat-note" style={{margin:'8px 0 0'}}>{tr('ui.toResendAnInvitationEnterYour')}</p>}
      {email&&!valid&&<p className="afya-stat-note" style={{margin:'8px 0 0'}}>{tr('ui.enterAPlainEmailAddressLetters')}</p>}
      {error&&<div className="afya-login-error" role="alert" style={{marginTop:12}}>{error}</div>}
      {result&&<div className="afya-notice info" role="status" style={{marginTop:12}}>
        <div><strong>{result.email_sent?tr('tpl.invitationEmailed', { email: result.invitation.email }):tr('tpl.invitationCreated', { email: result.invitation.email })}</strong>
          The link expires {eatDate(result.invitation.expires_at)}.
          {result.dev_invite_url&&<span style={{display:'block',marginTop:6}}>{tr('ui.developmentOnlyEmailIsNotSent')} <span className="afya-mono" style={{userSelect:'all',wordBreak:'break-all'}}>{result.dev_invite_url}</span></span>}
        </div></div>}
    </Card>

    {loadError&&<div className="afya-login-error" role="alert">{loadError} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>{tr('ui.retry')}</button></div>}

    <Card title={tr('tpl.invitationsCount', { count: invitations.length })}>
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>{tr('ui.email2')}</th><th>{tr('ui.invitedBy')}</th><th>{tr('ui.sent')}</th><th>{tr('ui.expires')}</th><th>{tr('ui.status')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
        <tbody>{invitations.map(i=><tr key={i.id}>
          <td style={{fontWeight:650}}>{i.email}</td><td>{i.invited_by??'—'}</td>
          <td style={{whiteSpace:'nowrap'}}>{eatDate(i.created_at)}</td><td style={{whiteSpace:'nowrap'}}>{i.status==='PENDING'?eatDate(i.expires_at):'—'}</td>
          <td><Chip tone={STATUS[i.status].tone}>{STATUS[i.status].label}</Chip></td>
          <td style={{textAlign:'right',whiteSpace:'nowrap'}}>{i.status==='PENDING'&&<>
            <button type="button" className="afya-btn-secondary" disabled={busy!==null||!hasPassword} title={hasPassword?undefined:tr('ui2.enterYourCurrentPasswordAboveTo')} onClick={()=>{const pw=password;void run(`resend:${i.id}`,()=>resendInvitation(i.id,pw));}}><RefreshCw size={15} aria-hidden/>{busy===`resend:${i.id}`?tr('ui2.sending'):tr('ui2.resend')}</button>
            <button type="button" className="afya-btn-danger" style={{marginLeft:8}} disabled={busy!==null} onClick={()=>{if(window.confirm(tr('tpl.revokeInvitation', { email: i.email })))void run(`revoke:${i.id}`,()=>revokeInvitation(i.id));}}><XCircle size={15} aria-hidden/>{tr('ui.revoke')}</button>
          </>}</td>
        </tr>)}</tbody>
      </table>{!loading&&!invitations.length&&<EmptyState title={tr('ui.noInvitationsYet')}>{tr('ui.inviteSomeoneAboveToAddThem')}</EmptyState>}</div>
    </Card>

    <Card title={tr('tpl.adminsCount', { count: admins.length })}>
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>{tr('ui.admin')}</th><th>{tr('ui.phone2')}</th><th>{tr('ui.invitedBy')}</th><th>{tr('ui.since')}</th><th>{tr('ui.status')}</th></tr></thead>
        <tbody>{admins.map(a=><tr key={a.id}>
          <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden><UserCheck size={16}/></span><span style={{fontWeight:650}}>{a.email??'—'}</span>{a.is_you&&<Chip tone="info">{tr('ui.you')}</Chip>}</div></td>
          <td><span className="afya-mono">{a.phone}</span></td><td>{a.invited_by??<span className="afya-stat-note">{tr('ui.foundingAdmin')}</span>}{a.invited_at&&<div className="afya-stat-note">{tr('mix.invitedOn', { date: eatDate(a.invited_at) })}</div>}</td>
          <td style={{whiteSpace:'nowrap'}}>{eatDate(a.created_at)}</td>
          <td><Chip tone={a.is_active?'success':'neutral'}>{a.is_active?tr('ui2.active'):tr('ui2.inactive')}</Chip></td>
        </tr>)}</tbody>
      </table>{!loading&&!admins.length&&<EmptyState title={tr('ui.noAdminsListed')}/>}</div>
    </Card>
  </div>;
}
