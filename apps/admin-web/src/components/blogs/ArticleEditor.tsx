'use client';
import {useEffect,useState} from 'react';
import type {AdminArticle,ArticleContent,ArticleCategory} from '../../lib/blogContent';
import {articleActions,articleErrors,blankArticle,editableArticle,nairobiSchedule} from '../../lib/blogContent';
import {saveArticle,transitionArticle,uploadCover,getArticle} from '../../lib/blogApi';
import {ArticlePreview} from './ArticlePreview';
import {McqBuilder} from './McqBuilder';
import type {QuizContent} from '../../lib/quizContent';
import {blogErrorMessage,independentApprovalMessage} from '../../lib/blogErrors';

export function ArticleEditor({initial,categories,onSaved,onClose}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:()=>void}){
  const [article,setArticle]=useState(initial);const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):blankArticle());
  const [id]=useState(()=>initial?.id||crypto.randomUUID());const [language,setLanguage]=useState<'en'|'sw'>('en');const [previewLanguage,setPreviewLanguage]=useState<'en'|'sw'>('en');
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');const [dirty,setDirty]=useState(!initial);
  const [schedule,setSchedule]=useState('');const [cover,setCover]=useState<File|null>(null);const [localCover,setLocalCover]=useState('');
  const [quizPreview,setQuizPreview]=useState<QuizContent|null>(null);const [quizDirty,setQuizDirty]=useState(false);
  useEffect(()=>{if(!cover)return;let active=true;const reader=new FileReader();reader.onload=()=>{if(active)setLocalCover(String(reader.result));};reader.readAsDataURL(cover);return()=>{active=false;reader.abort();};},[cover]);
  useEffect(()=>{const warn=(e:BeforeUnloadEvent)=>{if(dirty||cover||quizDirty){e.preventDefault();}};window.addEventListener('beforeunload',warn);return()=>window.removeEventListener('beforeunload',warn);},[dirty,cover,quizDirty]);
  function change<K extends keyof ArticleContent>(key:K,value:ArticleContent[K]){setContent(c=>({...c,[key]:value}));setDirty(true);setNotice('');}
  const archived=article?.status==='ARCHIVED';const errors=articleErrors(content);
  async function save(){
    setBusy(true);setError('');setNotice('');
    try{
      if(errors.length)throw new Error(errors.join(' '));
      let saved=await saveArticle(id,content,article?.version);setArticle(saved);setContent(editableArticle(saved));setDirty(false);
      if(cover){
        try{saved=await uploadCover(id,saved.version,cover);setArticle(saved);setContent(editableArticle(saved));setCover(null);}
        catch(e){setError(`Text draft saved. ${blogErrorMessage(e)}`);onSaved();return;}
      }
      setNotice('Draft saved. Preview remains private.');onSaved();
    }catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function act(action:string){
    if(!article||dirty||cover)return;
    if(action==='approve'&&article.can_approve!==true){setError(article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?independentApprovalMessage:'Approval permissions are unavailable. Reload the article.');return;}
    setBusy(true);setError('');setNotice('');
    try{const updated=await transitionArticle(article.id,article.version,action,action==='schedule'?nairobiSchedule(schedule):undefined);setArticle(updated);setContent(editableArticle(updated));setNotice(`${action==='return'?'Returned to draft':updated.status.toLowerCase().replaceAll('_',' ')}. No announcement was sent.`);onSaved();}
    catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function reload(){setBusy(true);try{const updated=await getArticle(id);setArticle(updated);setContent(editableArticle(updated));setDirty(false);setCover(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  function close(){if((dirty||cover||quizDirty)&&!window.confirm('Discard unsaved article or question changes?'))return;onClose();}
  return <div className="cms-editor">
    <div className="cms-editor-heading"><div><h2>{article?'Edit article':'New article'}</h2><p>{article?.status.replaceAll('_',' ')||'DRAFT'} · Content version {article?.content_version||1}</p></div><button type="button" onClick={close}>Back to articles</button></div>
    {error&&<div role="alert" className="cms-error">{error}{article&&<button type="button" onClick={reload}>Reload saved article</button>}{!article&&<button type="button" onClick={reload}>Recover saved draft</button>}</div>}
    {notice&&<p role="status" className="cms-notice">{notice}</p>}
    <div className="cms-editor-grid"><section className="cms-form">
      <fieldset disabled={busy||archived}><legend>Article content</legend>
        <div className="cms-language" role="group" aria-label="Editor language"><button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English</button><button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili</button></div>
        <label>{language==='en'?'English':'Kiswahili'} title<input value={content[`title_${language}`]} maxLength={160} onChange={e=>change(`title_${language}`,e.target.value)}/></label>
        <label>{language==='en'?'English':'Kiswahili'} content<textarea className="cms-body" value={content[`body_${language}`]} maxLength={30000} onChange={e=>change(`body_${language}`,e.target.value)} placeholder="## Heading\n\nWrite your article in Markdown…"/></label>
        <p className="cms-hint">Markdown supports headings, **bold**, *italic*, lists, blockquotes and [links](https://example.org). HTML and embedded media are disabled.</p>
        <label>Category<select value={content.category_id} onChange={e=>change('category_id',e.target.value)}><option value="">Choose category</option>{categories.map(c=><option key={c.id} value={c.id}>{c.name_en} / {c.name_sw}</option>)}</select></label>
        <label>Tags (comma separated)<input value={content.tags.join(',')} onChange={e=>change('tags',e.target.value?e.target.value.split(','):[])}/></label>
        <label className="cms-check"><input type="checkbox" checked={content.featured} onChange={e=>change('featured',e.target.checked)}/>Featured article</label>
        <label className="cms-check"><input type="checkbox" checked={content.sensitive} onChange={e=>change('sensitive',e.target.checked)}/>Sensitive health content — independent approval required</label>
        <p className="cms-hint">Medical, HIV prevention and adolescent-health content must use a category requiring independent review. A category’s review requirement also applies when this checkbox is off.</p>
        <label>Cover image (JPEG/PNG, up to 5 MB)<input type="file" accept="image/jpeg,image/png" onChange={e=>{const file=e.target.files?.[0]||null;if(file&&(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)){setError('Choose JPEG/PNG up to 5 MB.');return;}setCover(file);}}/></label>
        {cover&&<button type="button" onClick={()=>setCover(null)}>Remove selected cover</button>}
        <p className="cms-hint">Saving changes returns reviewed or published content to draft and clears its approval and schedule.</p>
        <button type="button" className="cms-primary" disabled={errors.length>0||(!dirty&&!cover)} onClick={save}>{busy?'Saving…':'Save draft'}</button>
        {errors.length>0&&<ul className="cms-hint">{errors.map(e=><li key={e}>{e}</li>)}</ul>}
      </fieldset>
      <McqBuilder article={article} onPreview={setQuizPreview} onDirty={setQuizDirty}/>
      {article&&!archived&&<fieldset disabled={busy||dirty||Boolean(cover)}><legend>Review & publication</legend>
        {article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?<p role="status" className="cms-hint">{independentApprovalMessage} This article remains pending review.</p>:article.requires_independent_review&&<p className="cms-hint">Independent review is required for this article.</p>}
        {article.approved_at&&<p className="cms-hint">Approved content version {article.approved_version} · {new Date(article.approved_at).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT · Reviewer {article.approved_by}</p>}
        {article.scheduled_for&&<p>Scheduled: {new Date(article.scheduled_for).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT</p>}
        {article.status==='APPROVED'&&<label>Publication date · Africa/Nairobi (UTC+3)<input type="datetime-local" value={schedule} onChange={e=>setSchedule(e.target.value)}/></label>}
        <div className="cms-actions">{articleActions(article.status).map(action=><button type="button" key={action} disabled={action==='approve'&&article.can_approve!==true} title={action==='approve'&&article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?independentApprovalMessage:undefined} onClick={()=>{if(action!=='archive'||window.confirm('Archive this article? Archived articles cannot be edited.'))void act(action);}}>{({submit:'Submit for review',approve:'Approve',return:'Return for revision',publish:'Publish now',schedule:'Schedule publication',unpublish:'Return to draft / unpublish',archive:'Archive'} as Record<string,string>)[action]}</button>)}</div>
        {(dirty||cover)&&<p>Save changes before review or publication.</p>}
      </fieldset>}
    </section><ArticlePreview content={content} category={categories.find(c=>c.id===content.category_id)} language={previewLanguage} setLanguage={setPreviewLanguage} article={article} localCover={cover?localCover:''} quiz={quizPreview}/></div>
  </div>;
}
