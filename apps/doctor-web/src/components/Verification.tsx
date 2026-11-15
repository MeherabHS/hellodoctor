'use client';
import { useRef, useState } from 'react';
import { errorText, post, useResource } from '../lib/client';
import { Profile, RecordDto } from '../lib/contracts';
import { State } from './Common';
import { Icon } from './Icons';
import { date } from './Queue';

type Prepared = { name: string; mime_type: string; base64_content: string; bytes: number };
const MAX_TOTAL = 9 * 1024 * 1024; // stays under the 10 MB Doctor Web proxy limit after JSON wrapping
async function prepare(file: File): Promise<Prepared> {
  if (file.type === 'application/pdf') {
    if (file.size > 4 * 1024 * 1024) throw new Error(`${file.name}: PDFs must be 4 MB or smaller.`);
    const data = await new Promise<string>((resolve, reject) => { const r = new FileReader(); r.onload = () => resolve(String(r.result).split(',')[1]); r.onerror = () => reject(new Error('Unable to read the file.')); r.readAsDataURL(file); });
    return { name: file.name, mime_type: 'application/pdf', base64_content: data, bytes: file.size };
  }
  if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type)) throw new Error(`${file.name}: use a PDF, JPEG, PNG or WebP file.`);
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 2000 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas'); canvas.width = Math.round(bitmap.width * scale); canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height); bitmap.close();
  const data = canvas.toDataURL('image/jpeg', 0.88).split(',')[1];
  return { name: file.name, mime_type: 'image/jpeg', base64_content: data, bytes: Math.round(data.length * 0.75) };
}

/** Shown while professional verification is PENDING or REJECTED. Clinical access stays closed.
 * Doctors can add documents, and a rejected application can be resubmitted for review. */
export function Verification({ profile, onRefresh, onLogout }: { profile: Profile; onRefresh: () => void; onLogout: () => void; onUpdated?: (profile: Profile) => void }) {
  const rejected = profile.professional_verification_status === 'REJECTED';
  const history = useResource<RecordDto[]>('doctor/verification/submissions');
  const [files, setFiles] = useState<Prepared[]>([]); const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [done, setDone] = useState(false); const [open, setOpen] = useState(rejected);
  const picker = useRef<HTMLInputElement>(null);
  const submissions = history.data ?? [];
  const steps: [string, string, 'done' | 'current' | 'failed' | ''][] = [
    ['Personal & professional details', 'Name, specialty and experience', 'done'],
    ['Professional registration', `KMPDC ${profile.license_number}`, 'done'],
    ['Supporting documents', submissions.length ? `${submissions.length} submission${submissions.length === 1 ? '' : 's'} sent` : 'Licence, ID or certificates (recommended)', submissions.length ? 'done' : ''],
    ['Review & verification', rejected ? 'Not approved' : 'AfyaMkononi is reviewing your application', rejected ? 'failed' : 'current'],
  ];
  async function add(list: File[]) {
    setError('');
    try {
      const next = [...files];
      for (const file of list) { if (next.length >= 5) throw new Error('Attach at most 5 documents.'); next.push(await prepare(file)); }
      if (next.reduce((sum, f) => sum + f.base64_content.length, 0) > MAX_TOTAL) throw new Error('These files are too large together. Remove one or use smaller files.');
      setFiles(next);
    } catch (e) { setError(e instanceof Error ? e.message : 'Unable to add this file.'); }
  }
  async function submit() {
    if (!files.length || busy) return;
    setBusy(true); setError('');
    try {
      await post('doctor/verification/submissions', { message: message.trim() || undefined, documents: files.map(({ mime_type, base64_content }) => ({ mime_type, base64_content })) });
      setDone(true); setFiles([]); setMessage(''); void history.refresh(); onRefresh();
    } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  }
  return <main style={{ minHeight: '100dvh', display: 'grid', placeItems: 'center', padding: 24 }}>
    <div className="am-card" style={{ width: '100%', maxWidth: 600 }}>
      <div className="am-card-body am-stack" style={{ gap: 20, padding: 32 }}>
        <div className="am-row" style={{ gap: 10 }}><img src="/brand/afya_logo.png" alt="" width={34} height={34}/><strong className="am-strong">AfyaMkononi Doctor Portal</strong></div>
        {done ? <div className="am-success" style={{ alignItems: 'flex-start', textAlign: 'left' }}>
          <span className="am-success-mark" style={{ width: 60, height: 60 }}><Icon name="check" size={30} stroke={2.6}/></span>
          <h1 style={{ fontSize: 24 }}>Documents submitted</h1>
          <p className="am-muted">Thank you. Your application is back in the review queue. We will notify you once it has been reviewed, usually within one to two business days.</p>
        </div> : rejected ? <div className="am-success" style={{ alignItems: 'flex-start', textAlign: 'left' }}>
          <span className="am-success-mark red" style={{ width: 60, height: 60 }}><Icon name="x" size={30} stroke={2.6}/></span>
          <h1 style={{ fontSize: 24 }}>Verification not approved</h1>
          <p className="am-muted">We could not verify your professional registration yet. Review the feedback, then send clearer or updated documents.</p>
        </div> : <div>
          <h1 style={{ fontSize: 24 }}>Professional verification</h1>
          <p className="am-muted" style={{ marginTop: 6 }}>Thank you, Dr. {profile.full_name}. We verify every practitioner before consultations open.</p>
        </div>}
        <div className="am-steps">{steps.map(([title, sub, state], i) => <div key={title} className={`am-step ${state}`}><span className="am-step-num">{state === 'done' ? <Icon name="check" size={17} stroke={2.6}/> : state === 'failed' ? <Icon name="x" size={17} stroke={2.6}/> : i + 1}</span><div><strong>{title}</strong><span>{sub}</span></div></div>)}</div>
        {rejected && !done && profile.professional_verification_note && <div className="am-callout red"><Icon name="alert"/><div><strong className="am-strong">Reason from the reviewer</strong><p className="am-pre" style={{ marginTop: 4 }}>{profile.professional_verification_note}</p></div></div>}
        {!rejected && !done && <div className="am-callout"><Icon name="clock"/><span><strong>Verification under review.</strong> We will notify you once your account is verified, usually within one to two business days.</span></div>}

        {!done && (open ? <section className="am-stack" style={{ gap: 12, padding: 18, border: '1px solid var(--am-line)', borderRadius: 16 }}>
          <div><strong className="am-strong">{rejected ? 'Resubmit documents' : 'Add supporting documents'}</strong><p className="am-faint">KMPDC licence, national ID or qualification certificates. PDF, JPEG or PNG, up to 5 files.</p></div>
          <input ref={picker} type="file" hidden multiple accept="application/pdf,image/jpeg,image/png,image/webp" aria-label="Verification documents" onChange={e => { const list = Array.from(e.target.files ?? []); e.target.value = ''; void add(list); }}/>
          {files.map((f, i) => <div key={i} className="am-file"><span className="am-file-icon"><Icon name={f.mime_type === 'application/pdf' ? 'file' : 'image'} size={19}/></span><div className="am-grow" style={{ minWidth: 0 }}><strong className="am-strong" style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{f.name}</strong><span className="am-faint">{Math.ceil(f.bytes / 1024)} KB</span></div><button className="am-btn ghost sm" aria-label={`Remove ${f.name}`} disabled={busy} onClick={() => setFiles(files.filter((_, j) => j !== i))}><Icon name="trash" size={14}/></button></div>)}
          {files.length < 5 && <button type="button" className="am-dropzone" disabled={busy} onClick={() => picker.current?.click()}><Icon name="upload" size={28}/><strong>Choose documents</strong><span>{files.length}/5 attached</span></button>}
          <label className="am-field"><span>Note for the reviewer (optional)</span><textarea className="am-textarea" rows={3} maxLength={1000} value={message} disabled={busy} placeholder="For example: attached a clearer copy of my licence, valid until 2027." onChange={e => setMessage(e.target.value)}/></label>
          <State compact error={error}/>
          <button className="am-btn primary block" disabled={busy || !files.length} onClick={() => void submit()}><Icon name="send" size={16}/>{busy ? 'Submitting…' : rejected ? 'Resubmit for review' : 'Send documents'}</button>
        </section> : <button className="am-btn soft block" onClick={() => setOpen(true)}><Icon name="upload" size={16}/>Add supporting documents</button>)}

        {submissions.length > 0 && <div className="am-stack" style={{ gap: 8 }}><span className="am-label">Your submissions</span>{submissions.map(s => <div key={String(s.id)} className="am-row between" style={{ padding: '10px 12px', border: '1px solid var(--am-line)', borderRadius: 12 }}><span>{date(s.created_at as string)} · {(s.documents as unknown[]).length} document(s)</span><span className="am-faint">{s.previous_status === 'REJECTED' ? 'Resubmission' : 'Added while pending'}</span></div>)}</div>}

        <div className="am-row"><button className="am-btn ghost am-grow" onClick={onRefresh}><Icon name="refresh" size={16}/>Check status again</button><button className="am-btn ghost" onClick={onLogout}><Icon name="logout" size={16}/>Log out</button></div>
      </div>
    </div>
  </main>;
}
