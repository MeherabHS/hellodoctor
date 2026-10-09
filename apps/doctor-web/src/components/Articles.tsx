'use client';
import { useEffect, useRef, useState } from 'react';
import { api, errorText } from '../lib/client';
import { articleActions, ArticleContent, articleValidation, DoctorArticle } from '../lib/articles';
import { State } from './Common';

export function Articles({specialty}:{specialty:string}) {
  const [rows,setRows]=useState<DoctorArticle[]>([]),[selected,setSelected]=useState<DoctorArticle|null>(null);
  const [editing,setEditing]=useState(false),[loading,setLoading]=useState(true),[busy,setBusy]=useState(false);
  const [error,setError]=useState(''),[notice,setNotice]=useState(''),[more,setMore]=useState(false);
  const [title,setTitle]=useState(''),[summary,setSummary]=useState(''),[body,setBody]=useState(''),[references,setReferences]=useState('');
  const creationKey=useRef(''),locked=useRef(false),alive=useRef(true);
  useEffect(()=>{alive.current=true;void load(0);return()=>{alive.current=false;};},[]);
  async function load(offset:number){
    setLoading(true);setError('');
    try{const page=await api<DoctorArticle[]>(`doctor/articles?limit=20&offset=${offset}`);if(alive.current){setRows(old=>offset?[...old,...page]:page);setMore(page.length===20);}}
    catch(e){if(alive.current)setError(errorText(e));}finally{if(alive.current)setLoading(false);}
  }
  function open(article:DoctorArticle|null){
    setSelected(article);setTitle(article?.title??'');setSummary(article?.summary??'');setBody(article?.body??'');
    setReferences(article?.references_list.join('\n')??'');creationKey.current=crypto.randomUUID();setEditing(true);setError('');setNotice('');
  }
  async function save(publish:boolean){
    if(locked.current)return;
    const content:ArticleContent={title:title.trim(),summary:summary.trim(),body:body.trim(),references_list:references.split('\n').map(r=>r.trim()).filter(Boolean)};
    const invalid=articleValidation(content);if(invalid){setError(invalid);return;}
    locked.current=true;setBusy(true);setError('');setNotice('');
    try{
      let article=await api<DoctorArticle>(selected?`doctor/articles/${selected.id}`:'doctor/articles',{
        method:selected?'PATCH':'POST',body:JSON.stringify(selected?{version:selected.version,content}:{creation_key:creationKey.current,content})});
      if(!alive.current)return;setSelected(article);
      if(publish){article=await api<DoctorArticle>(`doctor/articles/${article.id}/publish`,{method:'POST',body:JSON.stringify({version:article.version})});if(!alive.current)return;setSelected(article);}
      setNotice(publish?'Published. Your article is available in the patient feed.':article.status==='PUBLISHED'?'Published article updated.':'Draft saved.');
      await load(0);
    }catch(e){if(alive.current)setError(errorText(e));}finally{locked.current=false;if(alive.current)setBusy(false);}
  }
  async function archive(){
    if(!selected||locked.current||!window.confirm('Archive this article? It will leave the published feed.'))return;
    locked.current=true;setBusy(true);setError('');
    try{const a=await api<DoctorArticle>(`doctor/articles/${selected.id}/archive`,{method:'POST',body:JSON.stringify({version:selected.version})});if(!alive.current)return;setSelected(a);setNotice('Article archived.');await load(0);}
    catch(e){if(alive.current)setError(errorText(e));}finally{locked.current=false;if(alive.current)setBusy(false);}
  }
  const actions=articleActions(selected?.status??'DRAFT');
  return <section className="dw-page-view active web-articles-page" aria-label="My Articles">
    <header className="dw-page-header"><div className="dw-page-title-group"><h2>My Articles</h2><p>Share educational content related to {specialty}. You publish directly.</p></div><button className="dw-btn-outline" disabled={busy} onClick={()=>open(null)}>New article</button></header>
    <State loading={loading} error={error} retry={()=>void load(0)}/>{notice&&<p role="status">{notice}</p>}
    {editing&&<section className="dw-section-card"><div className="web-section-body">
      <p>{selected?.status??'DRAFT'} · {specialty}</p>
      <label>Title<input className="sheet-input" value={title} maxLength={160} disabled={busy||!actions.editable} onChange={e=>setTitle(e.target.value)}/></label>
      <label>Summary<textarea className="sheet-input" value={summary} maxLength={600} disabled={busy||!actions.editable} onChange={e=>setSummary(e.target.value)} rows={3}/></label>
      <label>Article<textarea className="sheet-input" value={body} maxLength={30000} disabled={busy||!actions.editable} onChange={e=>setBody(e.target.value)} rows={14}/></label>
      <label>References (optional, one per line)<textarea className="sheet-input" value={references} disabled={busy||!actions.editable} onChange={e=>setReferences(e.target.value)} rows={3}/></label>
      <p className="sheet-desc">Use general educational content. Do not include identifiable patient information. Articles do not replace an individual consultation.</p>
      <div className="web-action-row">
        {actions.editable&&<button className="sheet-btn secondary" disabled={busy} onClick={()=>void save(false)}>{selected?.status==='PUBLISHED'?'Update published article':'Save draft'}</button>}
        {actions.publishable&&<button className="sheet-btn primary" disabled={busy} onClick={()=>void save(true)}>Publish</button>}
        {selected&&actions.archivable&&<button className="dw-btn-outline" disabled={busy} onClick={()=>void archive()}>Archive</button>}
        {selected&&<button className="dw-btn-outline" disabled={busy} onClick={()=>{setBusy(true);void api<DoctorArticle>(`doctor/articles/${selected.id}`).then(a=>{if(alive.current)open(a);}).catch(e=>{if(alive.current)setError(errorText(e));}).finally(()=>{if(alive.current)setBusy(false);});}}>Reload latest</button>}
        <button className="dw-btn-outline" disabled={busy} onClick={()=>setEditing(false)}>Close</button>
      </div>
    </div></section>}
    {!loading&&!rows.length&&<p>No articles yet. Create your first draft.</p>}
    {rows.map(article=><article key={article.id} className="dw-section-card"><div className="web-section-body"><h3>{article.title}</h3><p>{article.summary}</p><p>{article.status} · {new Date(article.updated_at).toLocaleDateString()}</p><button className="dw-btn-outline" disabled={busy} onClick={()=>open(article)}>{article.status==='ARCHIVED'?'View':'Open article'}</button></div></article>)}
    {more&&<button className="dw-btn-outline" disabled={loading||busy} onClick={()=>void load(rows.length)}>Load more</button>}
  </section>;
}
