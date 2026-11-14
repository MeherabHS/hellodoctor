'use client';
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
const STATUS:Record<InvitationStatus,{tone:ChipTone;label:string}>={PENDING:{tone:'warn',label:'Pending'},ACCEPTED:{tone:'success',label:'Accepted'},REVOKED:{tone:'neutral',label:'Revoked'},EXPIRED:{tone:'danger',label:'Expired'}};

/** The backend's user-facing message from an "API Error [status]: {json}" error, never raw detail. */
function errorText(e:unknown):string{
  const m=e instanceof Error?e.message:'';const start=m.indexOf('{');
  try{const parsed=start>=0?JSON.parse(m.slice(start)):null;const msg=parsed?.error?.message;if(typeof msg==='string')return msg;}catch{/* fall through */}
  return 'The request could not be completed. Try again.';
}

export function AdminTeamPage(){
  const [admins,setAdmins]=useState<TeamAdmin[]>([]);const [invitations,setInvitations]=useState<TeamInvitation[]>([]);
  const [loading,setLoading]=useState(true);const [loadError,setLoadError]=useState('');const [revision,setRevision]=useState(0);
  const [email,setEmail]=useState('');const [password,setPassword]=useState('');const [busy,setBusy]=useState<string|null>(null);const [error,setError]=useState('');const [result,setResult]=useState<CreatedInvitation|null>(null);
  useEffect(()=>{let active=true;
    getTeam().then(t=>{if(active){setAdmins(t.admins);setInvitations(t.invitations);setLoadError('');}})
      .catch(()=>{if(active)setLoadError('The team could not be loaded.');})
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
    <PageHead title="Admin Team" subtitle="Invite people to become Admins. They set their own phone number and password from the emailed link."/>
    <div className="afya-stat-grid">
      <StatTile icon={Users} tone="green" value={loading?'—':admins.length} label="Admins"/>
      <StatTile icon={MailPlus} tone="amber" value={loading?'—':pending.length} label="Pending invitations"/>
      <StatTile icon={ShieldCheck} tone="blue" value="5 min" label="Invitation lifetime" note="Single use, revocable"/>
    </div>

    <Card title="Invite an Admin" subtitle="Only invite people who should have full Admin access. Inviting or resending asks for your password; every invitation and every wrong password is recorded in the Activity Log.">
      <form onSubmit={invite} className="afya-filter-bar" style={{marginBottom:0}} noValidate>
        <label className="afya-search" style={{flex:'1 1 320px'}}>
          <MailPlus size={16} color="var(--afya-faint)" aria-hidden/>
          <input type="email" aria-label="Email address to invite" placeholder="name@example.org" autoComplete="off" maxLength={254} value={email} onChange={e=>{setEmail(e.target.value);setError('');}}/>
        </label>
        <label className="afya-search" style={{flex:'0 1 260px'}}>
          <KeyRound size={16} color="var(--afya-faint)" aria-hidden/>
          <input type="password" aria-label="Your current password" placeholder="Your current password" autoComplete="current-password" maxLength={128} value={password} onChange={e=>{setPassword(e.target.value);setError('');}}/>
        </label>
        <button type="submit" className="afya-btn-primary" disabled={!valid||!hasPassword||busy!==null}><MailPlus size={16} aria-hidden/>{busy==='invite'?'Sending…':'Send invitation'}</button>
      </form>
      {!hasPassword&&pending.length>0&&<p className="afya-stat-note" style={{margin:'8px 0 0'}}>To resend an invitation, enter your current password above first.</p>}
      {email&&!valid&&<p className="afya-stat-note" style={{margin:'8px 0 0'}}>Enter a plain email address (letters, digits and . _ % + - only).</p>}
      {error&&<div className="afya-login-error" role="alert" style={{marginTop:12}}>{error}</div>}
      {result&&<div className="afya-notice info" role="status" style={{marginTop:12}}>
        <div><strong>{result.email_sent?`Invitation emailed to ${result.invitation.email}`:`Invitation created for ${result.invitation.email}`}</strong>
          The link expires {eatDate(result.invitation.expires_at)}.
          {result.dev_invite_url&&<span style={{display:'block',marginTop:6}}>Development only (email is not sent here): <span className="afya-mono" style={{userSelect:'all',wordBreak:'break-all'}}>{result.dev_invite_url}</span></span>}
        </div></div>}
    </Card>

    {loadError&&<div className="afya-login-error" role="alert">{loadError} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}

    <Card title={`Invitations (${invitations.length})`}>
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>Email</th><th>Invited by</th><th>Sent</th><th>Expires</th><th>Status</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
        <tbody>{invitations.map(i=><tr key={i.id}>
          <td style={{fontWeight:650}}>{i.email}</td><td>{i.invited_by??'—'}</td>
          <td style={{whiteSpace:'nowrap'}}>{eatDate(i.created_at)}</td><td style={{whiteSpace:'nowrap'}}>{i.status==='PENDING'?eatDate(i.expires_at):'—'}</td>
          <td><Chip tone={STATUS[i.status].tone}>{STATUS[i.status].label}</Chip></td>
          <td style={{textAlign:'right',whiteSpace:'nowrap'}}>{i.status==='PENDING'&&<>
            <button type="button" className="afya-btn-secondary" disabled={busy!==null||!hasPassword} title={hasPassword?undefined:'Enter your current password above to resend'} onClick={()=>{const pw=password;void run(`resend:${i.id}`,()=>resendInvitation(i.id,pw));}}><RefreshCw size={15} aria-hidden/>{busy===`resend:${i.id}`?'Sending…':'Resend'}</button>
            <button type="button" className="afya-btn-danger" style={{marginLeft:8}} disabled={busy!==null} onClick={()=>{if(window.confirm(`Revoke the invitation for ${i.email}? The link stops working.`))void run(`revoke:${i.id}`,()=>revokeInvitation(i.id));}}><XCircle size={15} aria-hidden/>Revoke</button>
          </>}</td>
        </tr>)}</tbody>
      </table>{!loading&&!invitations.length&&<EmptyState title="No invitations yet">Invite someone above to add them to the Admin team.</EmptyState>}</div>
    </Card>

    <Card title={`Admins (${admins.length})`}>
      <div className="afya-table-wrap"><table className="admin-table">
        <thead><tr><th>Admin</th><th>Phone</th><th>Invited by</th><th>Since</th><th>Status</th></tr></thead>
        <tbody>{admins.map(a=><tr key={a.id}>
          <td><div style={{display:'flex',alignItems:'center',gap:10}}><span className="afya-avatar" aria-hidden><UserCheck size={16}/></span><span style={{fontWeight:650}}>{a.email??'—'}</span>{a.is_you&&<Chip tone="info">You</Chip>}</div></td>
          <td><span className="afya-mono">{a.phone}</span></td><td>{a.invited_by??<span className="afya-stat-note">Founding Admin</span>}{a.invited_at&&<div className="afya-stat-note">Invited {eatDate(a.invited_at)}</div>}</td>
          <td style={{whiteSpace:'nowrap'}}>{eatDate(a.created_at)}</td>
          <td><Chip tone={a.is_active?'success':'neutral'}>{a.is_active?'Active':'Inactive'}</Chip></td>
        </tr>)}</tbody>
      </table>{!loading&&!admins.length&&<EmptyState title="No Admins listed"/>}</div>
    </Card>
  </div>;
}
