'use client';
import { useEffect, useRef, useState } from 'react';
import { api, errorText, post } from '../lib/client';
import { ARTICLE_TOPICS, articleActions, ArticleContent, articleValidation, DoctorArticle, topicLabel } from '../lib/articles';
import { Badge, PageHeader, State, Stat, type Tone } from './Common';
import { Icon } from './Icons';
import { date } from './Queue';

const statusTone: Record<string, [string, Tone]> = { DRAFT: ['Draft', 'amber'], PUBLISHED: ['Published', 'green'], ARCHIVED: ['Archived', 'slate'] };
type Filter = 'all' | 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';

/** Cover image through the binary BFF route; the object URL is revoked on change. */
function useCover(article: DoctorArticle | null) {
  const [url, setUrl] = useState<string | undefined>();
  const key = article?.cover_image_id ? `${article.id}:${article.cover_image_id}` : null;
  useEffect(() => {
    if (!key) { setUrl(undefined); return; }
    let objectUrl = ''; let cancelled = false;
    fetch(`/api/article-cover/${key.split(':')[0]}`, { credentials: 'same-origin', cache: 'no-store' })
      .then(async res => { if (!res.ok || cancelled) return; objectUrl = URL.createObjectURL(await res.blob()); if (!cancelled) setUrl(objectUrl); }).catch(() => {});
    return () => { cancelled = true; if (objectUrl) URL.revokeObjectURL(objectUrl); setUrl(undefined); };
  }, [key]);
  return url;
}
function Cover({ article, large }: { article: DoctorArticle | null; large?: boolean }) {
  const url = useCover(article);
  return <span className={`am-cover${large ? ' lg' : ''}`}>{url ? <img src={url} alt=""/> : <Icon name="book" size={large ? 40 : 20}/>}</span>;
}
/** Resize to at most 1600 px and re-encode as JPEG in the browser; the server re-encodes again. */
async function prepareCover(file: File) {
  if (!['image/jpeg', 'image/png', 'image/webp'].includes(file.type)) throw new Error('Choose a JPEG, PNG or WebP image.');
  if (file.size > 15 * 1024 * 1024) throw new Error('Choose an image smaller than 15 MB.');
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 1600 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas'); canvas.width = Math.round(bitmap.width * scale); canvas.height = Math.round(bitmap.height * scale);
  canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height); bitmap.close();
  return { mime_type: 'image/jpeg', base64_content: canvas.toDataURL('image/jpeg', 0.85).split(',')[1] };
}

export function Articles({ specialty }: { specialty: string }) {
  const [rows, setRows] = useState<DoctorArticle[]>([]), [selected, setSelected] = useState<DoctorArticle | null>(null);
  const [editing, setEditing] = useState(false), [loading, setLoading] = useState(true), [busy, setBusy] = useState(false);
  const [error, setError] = useState(''), [notice, setNotice] = useState(''), [more, setMore] = useState(false);
  const [title, setTitle] = useState(''), [summary, setSummary] = useState(''), [body, setBody] = useState(''), [references, setReferences] = useState('');
  const [topic, setTopic] = useState(''), [titleSw, setTitleSw] = useState(''), [summarySw, setSummarySw] = useState(''), [bodySw, setBodySw] = useState('');
  const [language, setLanguage] = useState<'en' | 'sw'>('en');
  const [filter, setFilter] = useState<Filter>('all'), [search, setSearch] = useState(''), [preview, setPreview] = useState(false);
  const creationKey = useRef(''), locked = useRef(false), alive = useRef(true), coverPicker = useRef<HTMLInputElement>(null);
  useEffect(() => { alive.current = true; void load(0); return () => { alive.current = false; }; }, []);
  async function load(offset: number) {
    setLoading(true); setError('');
    try { const page = await api<DoctorArticle[]>(`doctor/articles?limit=20&offset=${offset}`); if (alive.current) { setRows(old => offset ? [...old, ...page] : page); setMore(page.length === 20); } }
    catch (e) { if (alive.current) setError(errorText(e)); } finally { if (alive.current) setLoading(false); }
  }
  function open(article: DoctorArticle | null) {
    setSelected(article); setTitle(article?.title ?? ''); setSummary(article?.summary ?? ''); setBody(article?.body ?? '');
    setReferences(article?.references_list.join('\n') ?? ''); setTopic(article?.topic_key ?? '');
    setTitleSw(article?.title_sw ?? ''); setSummarySw(article?.summary_sw ?? ''); setBodySw(article?.body_sw ?? ''); setLanguage('en');
    creationKey.current = crypto.randomUUID(); setEditing(true); setPreview(false); setError(''); setNotice('');
  }
  function content(): ArticleContent {
    const hasSw = !!(titleSw.trim() || summarySw.trim() || bodySw.trim());
    return { title: title.trim(), summary: summary.trim(), body: body.trim(), references_list: references.split('\n').map(r => r.trim()).filter(Boolean),
      extras: { topic_key: topic || null, swahili: hasSw ? { title: titleSw.trim(), summary: summarySw.trim(), body: bodySw.trim() } : null } };
  }
  async function save(publish: boolean) {
    if (locked.current) return;
    const value = content();
    const invalid = articleValidation(value); if (invalid) { setError(invalid); if (invalid.startsWith('Swahili')) setLanguage('sw'); return; }
    locked.current = true; setBusy(true); setError(''); setNotice('');
    try {
      let article = await api<DoctorArticle>(selected ? `doctor/articles/${selected.id}` : 'doctor/articles', { method: selected ? 'PATCH' : 'POST', body: JSON.stringify(selected ? { version: selected.version, content: value } : { creation_key: creationKey.current, content: value }) });
      if (!alive.current) return; setSelected(article);
      if (publish) { article = await api<DoctorArticle>(`doctor/articles/${article.id}/publish`, { method: 'POST', body: JSON.stringify({ version: article.version }) }); if (!alive.current) return; setSelected(article); }
      setNotice(publish ? 'Published. Your article is now in the patient feed.' : article.status === 'PUBLISHED' ? 'Published article updated.' : 'Draft saved.');
      await load(0);
    } catch (e) { if (alive.current) setError(errorText(e)); } finally { locked.current = false; if (alive.current) setBusy(false); }
  }
  async function uploadCover(file: File) {
    if (!selected || locked.current) return;
    locked.current = true; setBusy(true); setError(''); setNotice('');
    try { const article = await post<DoctorArticle>(`doctor/articles/${selected.id}/cover`, { version: selected.version, ...(await prepareCover(file)) }); if (!alive.current) return; setSelected(article); setNotice('Cover image updated.'); await load(0); }
    catch (e) { if (alive.current) setError(e instanceof Error && !('status' in e) ? e.message : errorText(e)); } finally { locked.current = false; if (alive.current) setBusy(false); }
  }
  async function archive() {
    if (!selected || locked.current || !window.confirm('Archive this article? It will leave the patient feed.')) return;
    locked.current = true; setBusy(true); setError('');
    try { const a = await api<DoctorArticle>(`doctor/articles/${selected.id}/archive`, { method: 'POST', body: JSON.stringify({ version: selected.version }) }); if (!alive.current) return; setSelected(a); setNotice('Article archived.'); await load(0); }
    catch (e) { if (alive.current) setError(errorText(e)); } finally { locked.current = false; if (alive.current) setBusy(false); }
  }
  const actions = articleActions(selected?.status ?? 'DRAFT');
  if (editing) {
    const [label, tone] = statusTone[selected?.status ?? 'DRAFT'];
    const ro = busy || !actions.editable;
    const sw = language === 'sw';
    const hasSw = !!(titleSw.trim() || summarySw.trim() || bodySw.trim());
    return <section className="am-stack" style={{ gap: 22 }} aria-label="Article editor">
      <PageHeader title={selected ? 'Edit article' : 'New article'} subtitle={`Health education for patients · ${specialty}`} back={{ label: 'My articles', onClick: () => setEditing(false) }}>
        <button className="am-btn ghost" onClick={() => setPreview(p => !p)}><Icon name="eye" size={16}/>{preview ? 'Back to editing' : 'Preview'}</button>
      </PageHeader>
      <div className="am-grid main-side" style={{ alignItems: 'start' }}>
        <section className="am-card">
          <div className="am-card-head">
            <div className="am-tabs" role="tablist" aria-label="Article language">
              <button className="am-tab" role="tab" aria-selected={!sw} onClick={() => setLanguage('en')}><Icon name="globe" size={15}/>English</button>
              <button className="am-tab" role="tab" aria-selected={sw} onClick={() => setLanguage('sw')}><Icon name="globe" size={15}/>Kiswahili{hasSw ? '' : ' (optional)'}</button>
            </div>
            {topicLabel(topic) && <Badge tone="teal">{topicLabel(topic)}</Badge>}
          </div>
          {preview ? <article className="am-card-body am-stack" style={{ gap: 14, padding: '28px 32px' }}>
            <Cover article={selected} large/>
            <h2 style={{ fontSize: 26 }}>{(sw ? titleSw : title) || 'Untitled article'}</h2>
            <p className="am-muted" style={{ fontSize: 15.5 }}>{sw ? summarySw : summary}</p>
            <p className="am-pre" style={{ fontSize: 15, lineHeight: 1.7 }}>{sw ? bodySw : body}</p>
            {!sw && references.trim() && <div><span className="am-label">References</span><ol style={{ paddingLeft: 18, color: 'var(--am-muted)' }}>{references.split('\n').filter(r => r.trim()).map((r, i) => <li key={i}>{r}</li>)}</ol></div>}
          </article> : sw ? <div className="am-card-body am-form">
            <div className="am-callout blue"><Icon name="info"/><span>Patients who use the app in Kiswahili see this version. Fill in all three fields, or leave them all empty.</span></div>
            <label className="am-field"><span>Kichwa (title)</span><input className="am-input" value={titleSw} maxLength={160} disabled={ro} onChange={e => setTitleSw(e.target.value)}/><small>{titleSw.length}/160</small></label>
            <label className="am-field"><span>Muhtasari (summary)</span><textarea className="am-textarea" value={summarySw} maxLength={600} rows={3} disabled={ro} onChange={e => setSummarySw(e.target.value)}/><small>{summarySw.length}/600</small></label>
            <label className="am-field"><span>Makala (article)</span><textarea className="am-textarea" value={bodySw} maxLength={30000} rows={16} disabled={ro} onChange={e => setBodySw(e.target.value)}/><small>{bodySw.length}/30000</small></label>
          </div> : <div className="am-card-body am-form">
            <label className="am-field"><span>Title</span><input className="am-input" value={title} maxLength={160} disabled={ro} placeholder="For example: Understanding PrEP" onChange={e => setTitle(e.target.value)}/><small>{title.length}/160</small></label>
            <label className="am-field"><span>Short summary</span><textarea className="am-textarea" value={summary} maxLength={600} rows={3} disabled={ro} placeholder="One or two sentences shown in the patient feed." onChange={e => setSummary(e.target.value)}/><small>{summary.length}/600</small></label>
            <label className="am-field"><span>Article</span><textarea className="am-textarea" value={body} maxLength={30000} rows={16} disabled={ro} placeholder="Write in plain, friendly language." onChange={e => setBody(e.target.value)}/><small>{body.length}/30000</small></label>
            <label className="am-field"><span>References (optional, one per line)</span><textarea className="am-textarea" value={references} rows={3} disabled={ro} onChange={e => setReferences(e.target.value)}/></label>
          </div>}
        </section>
        <div className="am-stack" style={{ gap: 18 }}>
          <section className="am-card"><div className="am-card-head"><h2>Publishing</h2><Badge tone={tone} dot>{label}</Badge></div>
            <div className="am-card-body am-stack">
              <label className="am-field"><span>Topic</span><select className="am-select" value={topic} disabled={ro} onChange={e => setTopic(e.target.value)}><option value="">No specific topic</option>{ARTICLE_TOPICS.map(([key, name]) => <option key={key} value={key}>{name}</option>)}</select><small>Helps patients find it under Prevention &amp; Education.</small></label>
              {selected && <dl className="am-details" style={{ gridTemplateColumns: 'auto 1fr' }}><dt>Last saved</dt><dd>{date(selected.updated_at)}</dd>{selected.published_at && <><dt>Published</dt><dd>{date(selected.published_at)}</dd></>}</dl>}
              <State error={error}/>{notice && <p className="am-ok-text" role="status"><Icon name="checkCircle" size={16}/>{notice}</p>}
              {actions.editable && <button className="am-btn ghost block" disabled={busy} onClick={() => void save(false)}><Icon name="file" size={16}/>{selected?.status === 'PUBLISHED' ? 'Update published article' : 'Save draft'}</button>}
              {actions.publishable && <button className="am-btn primary block" disabled={busy} onClick={() => void save(true)}><Icon name="send" size={16}/>Publish</button>}
              {selected && actions.archivable && <button className="am-btn danger block" disabled={busy} onClick={() => void archive()}><Icon name="archive" size={16}/>Archive</button>}
            </div></section>
          <section className="am-card"><div className="am-card-head"><h2><Icon name="image"/>Cover image</h2></div>
            <div className="am-card-body am-stack">
              <Cover article={selected} large/>
              <input ref={coverPicker} type="file" hidden accept="image/jpeg,image/png,image/webp" aria-label="Cover image file" onChange={e => { const f = e.target.files?.[0]; e.target.value = ''; if (f) void uploadCover(f); }}/>
              <button className="am-btn soft block" disabled={busy || !selected || !actions.editable} onClick={() => coverPicker.current?.click()}><Icon name="upload" size={16}/>{selected?.cover_image_id ? 'Change cover' : 'Upload cover'}</button>
              <p className="am-faint">{selected ? 'A calm, relevant photo or illustration. JPEG, PNG or WebP.' : 'Save the draft first, then add a cover.'}</p>
            </div></section>
          <div className="am-callout amber"><Icon name="info"/><span>Use general educational content. Never include identifiable patient information. Articles do not replace an individual consultation.</span></div>
        </div>
      </div>
    </section>;
  }
  const term = search.trim().toLowerCase();
  const list = rows.filter(a => (filter === 'all' || a.status === filter) && (!term || `${a.title} ${a.summary} ${a.title_sw ?? ''}`.toLowerCase().includes(term)));
  const count = (s: string) => rows.filter(a => a.status === s).length;
  return <>
    <PageHeader title="My articles" subtitle={`Share health education related to ${specialty}. You publish directly to the patient feed.`}>
      <button className="am-btn primary" disabled={busy} onClick={() => open(null)}><Icon name="plus" size={17}/>New article</button>
    </PageHeader>
    <div className="am-grid stats">
      <Stat icon="book" label="Total articles" value={`${rows.length}${more ? '+' : ''}`}/>
      <Stat icon="checkCircle" tone="teal" label="Published" value={count('PUBLISHED')}/>
      <Stat icon="pen" tone="amber" label="Drafts" value={count('DRAFT')}/>
      <Stat icon="archive" tone="violet" label="Archived" value={count('ARCHIVED')}/>
    </div>
    <section className="am-card">
      <div className="am-card-head">
        <label className="am-search" style={{ width: 300, maxWidth: '100%' }}><Icon name="search" size={16}/><span className="am-sr">Search articles</span><input value={search} placeholder="Search by title" onChange={e => setSearch(e.target.value)}/></label>
        <div className="am-tabs" aria-label="Status filter">{([['all', 'All'], ['PUBLISHED', 'Published'], ['DRAFT', 'Drafts'], ['ARCHIVED', 'Archived']] as [Filter, string][]).map(([value, label]) => <button key={value} className="am-tab" aria-pressed={filter === value} onClick={() => setFilter(value)}>{label}</button>)}</div>
      </div>
      <State loading={loading && !rows.length} error={error} retry={() => void load(0)} empty={!loading && !error && !list.length ? (rows.length ? 'No matching articles' : 'No articles yet') : undefined} icon="book" hint={rows.length ? undefined : 'Write your first article to help patients understand their health.'}/>
      {list.length > 0 && <div className="am-table-wrap"><table className="am-table">
        <thead><tr><th>Article</th><th>Topic</th><th>Status</th><th>Updated</th><th style={{ textAlign: 'right' }}>Actions</th></tr></thead>
        <tbody>{list.map(a => { const [label, tone] = statusTone[a.status]; return <tr key={a.id} style={{ cursor: 'pointer' }} onClick={() => open(a)}>
          <td><div className="am-row" style={{ flexWrap: 'nowrap', gap: 14 }}><Cover article={a}/><div style={{ minWidth: 0 }}><strong className="am-strong" style={{ display: 'block' }}>{a.title}</strong><span className="am-faint am-clamp">{a.summary}</span>{a.title_sw && <span className="am-faint" style={{ display: 'block' }}><Icon name="globe" size={12}/> Kiswahili version</span>}</div></div></td>
          <td>{topicLabel(a.topic_key) ? <Badge tone="teal">{topicLabel(a.topic_key)}</Badge> : <span className="am-faint">—</span>}</td>
          <td><Badge tone={tone} dot>{label}</Badge></td>
          <td className="am-muted nowrap">{date(a.updated_at)}</td>
          <td><div className="am-row end"><button className="am-btn ghost sm" onClick={e => { e.stopPropagation(); open(a); }}><Icon name={a.status === 'ARCHIVED' ? 'eye' : 'pen'} size={15}/>{a.status === 'ARCHIVED' ? 'View' : 'Edit'}</button></div></td>
        </tr>; })}</tbody>
      </table></div>}
      {more && <div className="am-card-body am-row" style={{ justifyContent: 'center' }}><button className="am-btn ghost" disabled={loading || busy} onClick={() => void load(rows.length)}>Load more</button></div>}
    </section>
  </>;
}
