'use client';
import { useRef, useState } from 'react';
import { api, errorText, post, useResource } from '../lib/client';
import { Profile, Question } from '../lib/contracts';
import { Avatar, Badge, PageHeader, State, Stat } from './Common';
import { Icon } from './Icons';
import { date, time } from './Queue';

const presets = [['Hydration & ORS', 'Oral hydration with ORS + Paracetamol 500mg (max 4/day). Avoid antibiotics without blood test.'], ['Fasting lab test', 'Fasting blood glucose & HbA1c test recommended at a verified lab. Cut refined sugars.'], ['Child care', 'Keep room humidified and provide warm fluids. If symptoms worsen, please visit hospital.']];
type Filter = 'all' | 'open' | 'answered';

export function Qna({ profile, onChanged }: { profile: Profile; onChanged: () => void }) {
  const questions = useResource<Question[]>('doctor/qna/questions?status=ALL&limit=100&offset=0');
  const [filter, setFilter] = useState<Filter>('open'); const [search, setSearch] = useState(''); const [paging, setPaging] = useState(false); const [pageError, setPageError] = useState(''); const [hasMore, setHasMore] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const all = questions.data ?? [], open = all.filter(q => q.status === 'OPEN'), answered = all.filter(q => q.status === 'ANSWERED');
  const term = search.trim().toLowerCase();
  const filtered = all.filter(q => (filter === 'all' || q.status === (filter === 'open' ? 'OPEN' : 'ANSWERED')) && (!term || `${q.question_text} ${q.category ?? ''} ${q.answer_text ?? ''}`.toLowerCase().includes(term)));
  async function more() { if (paging) return; setPaging(true); try { const page = await api<Question[]>(`doctor/qna/questions?status=ALL&limit=100&offset=${all.length}`); questions.setData([...all, ...page]); setHasMore(page.length === 100); } catch (e) { setPageError(errorText(e)); } finally { setPaging(false); } }
  const selected = all.find(q => q.id === selectedId) ?? null;
  const partial = hasMore && all.length >= 100;
  const n = (value: number) => questions.data ? `${value}${partial ? '+' : ''}` : '—';
  const refresh = () => { void questions.refresh(); onChanged(); };
  return <>
    <PageHeader title="Free health Q&A" subtitle="Answer anonymous questions from the community. Answers are published publicly under your name.">
      <Badge tone="green"><Icon name="shield" size={13}/>KMPDC {profile.license_number.replace(/^KMPDC\s*/i, '')}</Badge>
      <button className="am-btn ghost" onClick={refresh}><Icon name="refresh" size={16}/>Refresh</button>
    </PageHeader>
    <div className="am-grid three">
      <Stat icon="hourglass" tone="amber" label="Awaiting an answer" value={n(open.length)} sub="Open community questions"/>
      <Stat icon="checkCircle" label="Answered" value={n(answered.length)} sub="Published to the community"/>
      <Stat icon="help" tone="teal" label="All questions" value={n(all.length)}/>
    </div>
    <div className={selected ? 'am-grid main-side' : 'am-stack'} style={{ alignItems: 'start' }}>
      <section className="am-card">
        <div className="am-card-head">
          <div className="am-tabs" aria-label="Question filter">{([['open', 'Awaiting answer', open.length], ['answered', 'Answered', answered.length], ['all', 'All', all.length]] as [Filter, string, number][]).map(([value, label, count]) => <button key={value} className="am-tab" aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}<span className="am-tab-count">{count}</span></button>)}</div>
          <label className="am-search" style={{ width: 300, maxWidth: '100%' }}><Icon name="search" size={16}/><span className="am-sr">Search questions</span><input value={search} placeholder="Search, e.g. fever, PrEP, pregnancy" onChange={e => setSearch(e.target.value)}/></label>
        </div>
        <State loading={questions.loading && !questions.data} error={questions.error || pageError} retry={() => void questions.refresh()} empty={questions.data && !filtered.length ? (filter === 'open' && !term ? 'No open questions right now' : 'No matching questions') : undefined} icon="help" hint={filter === 'open' && !term ? 'Thank you. Every question has been answered.' : undefined}/>
        <div>{filtered.map(q => <article key={q.id} className="am-question" data-question-id={q.id} style={{ cursor: 'pointer', background: selectedId === q.id ? 'var(--am-green-mist)' : undefined }} onClick={() => setSelectedId(q.id)}>
          <div className="am-row between"><div className="am-row" style={{ gap: 8 }}><Badge tone="teal">{q.category ?? 'General health'}</Badge>{q.status === 'ANSWERED' ? <Badge tone="green" dot>Answered</Badge> : <Badge tone="amber" dot>Awaiting answer</Badge>}</div><span className="am-faint">Anonymous · {date(q.created_at)}</span></div>
          <h3>{q.question_text}</h3>
          {q.status === 'ANSWERED' ? <p className="am-faint">Answered by {q.answering_doctor}</p> : <button className="am-link" onClick={e => { e.stopPropagation(); setSelectedId(q.id); }}>Write an answer<Icon name="arrowRight" size={15}/></button>}
        </article>)}</div>
        {partial && <div className="am-card-body am-row" style={{ justifyContent: 'center' }}><button className="am-btn ghost" disabled={paging} onClick={() => void more()}>{paging ? 'Loading…' : 'Load more questions'}</button></div>}
      </section>
      {selected && <AnswerPanel key={selected.id} question={selected} profile={profile} onClose={() => setSelectedId(null)} onAnswered={refresh}/>}
    </div>
  </>;
}

function AnswerPanel({ question: q, profile, onClose, onAnswered }: { question: Question; profile: Profile; onClose: () => void; onAnswered: () => void }) {
  const [answer, setAnswer] = useState(''); const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [done, setDone] = useState(false); const lock = useRef(false);
  async function publish() { if (lock.current || !answer.trim()) return; lock.current = true; setBusy(true); setError(''); try { await post(`doctor/qna/questions/${q.id}/answers`, { answer_text: answer }); setDone(true); onAnswered(); } catch (e) { setError(errorText(e)); if (e instanceof Error && 'status' in e && e.status === 409) onAnswered(); } finally { lock.current = false; setBusy(false); } }
  return <section className="am-card" style={{ position: 'sticky', top: 'calc(var(--am-topbar-h) + 20px)' }} aria-label="Question detail">
    <div className="am-card-head"><h2><Icon name="help"/>Question</h2><button className="am-close" aria-label="Close question" onClick={onClose}><Icon name="close"/></button></div>
    <div className="am-card-body am-stack">
      <div className="am-row" style={{ gap: 8 }}><Badge tone="teal">{q.category ?? 'General health'}</Badge><span className="am-faint">Asked {date(q.created_at)}, {time(q.created_at)}</span></div>
      <div className="am-callout" style={{ background: 'var(--am-head)', borderColor: 'var(--am-line)' }}><Icon name="user"/><div><strong className="am-strong">Anonymous patient</strong><p style={{ marginTop: 4, fontSize: 15, color: 'var(--am-ink)' }}>{q.question_text}</p></div></div>
      {q.status === 'ANSWERED' ? <div className="am-answer"><div className="am-person" style={{ marginBottom: 8 }}><Avatar name={q.answering_doctor ?? 'Doctor'} size="sm"/><div><strong>{q.answering_doctor}</strong><span>Published answer</span></div></div><p className="am-pre">{q.answer_text}</p></div>
        : done ? <div className="am-success" role="status"><span className="am-success-mark"><Icon name="check" size={34} stroke={2.6}/></span><strong className="am-strong">Answer published</strong><p className="am-muted">Thank you for supporting community health.</p></div>
        : <>
          <div className="am-person"><Avatar name={profile.full_name} size="sm"/><div><strong>{profile.full_name}</strong><span>Your answer will show your name</span></div></div>
          <div><span className="am-label">Quick start</span><div className="am-row" style={{ marginTop: 6, gap: 6 }}>{presets.map(([label, value]) => <button key={label} className="am-btn soft sm" disabled={busy} onClick={() => setAnswer(value)}>{label}</button>)}</div></div>
          <label className="am-field"><span>Your answer</span><textarea id={`answer-${q.id}`} className="am-textarea" rows={7} maxLength={4000} value={answer} disabled={busy} onChange={e => setAnswer(e.target.value)} placeholder="Write clear, general guidance. Advise seeing a clinician when needed."/><small>{answer.length}/4000 · Do not ask for or include personal details.</small></label>
          <State error={error}/>
          <div className="am-row end"><button className="am-btn ghost" onClick={onClose} disabled={busy}>Cancel</button><button className="am-btn primary" disabled={busy || !answer.trim()} onClick={() => void publish()}><Icon name="send" size={16}/>{busy ? 'Publishing…' : 'Publish answer'}</button></div>
        </>}
    </div>
  </section>;
}
