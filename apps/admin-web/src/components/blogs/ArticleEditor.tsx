'use client';
import {useEffect,useRef,useState} from 'react';
import type {AdminArticle,ArticleContent,ArticleCategory} from '../../lib/blogContent';
import {articleActions,articleErrors,blankArticle,editableArticle,nairobiSchedule} from '../../lib/blogContent';
import {saveArticle,transitionArticle,uploadCover,getArticle} from '../../lib/blogApi';
import {ArticlePreview} from './ArticlePreview';
import {McqBuilder} from './McqBuilder';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import type {QuizContent} from '../../lib/quizContent';
import {blogErrorMessage,independentApprovalMessage} from '../../lib/blogErrors';

/** `mode='quiz'`: a standalone quiz (Quiz Manager). Same review and publish rules;
 * the content is a short introduction and there is no video link. */
export function ArticleEditor({initial,categories,onSaved,onClose,mode='article'}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:()=>void;mode?:'article'|'quiz'}){
  const quiz=mode==='quiz'||initial?.standalone_quiz===true;
  const [article,setArticle]=useState(initial);const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):blankArticle());
  const [id]=useState(()=>initial?.id||crypto.randomUUID());const [language,setLanguage]=useState<'en'|'sw'>('en');const [previewLanguage,setPreviewLanguage]=useState<'en'|'sw'>('en');
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');const [dirty,setDirty]=useState(false);
  const [schedule,setSchedule]=useState('');const [cover,setCover]=useState<File|null>(null);const [localCover,setLocalCover]=useState('');
  const [quizPreview,setQuizPreview]=useState<QuizContent|null>(null);const [quizDirty,setQuizDirty]=useState(false);
  const [quizBusy,setQuizBusy]=useState(false);const baseline=useRef(JSON.stringify(initial?editableArticle(initial):blankArticle()));
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty||Boolean(cover)||quizDirty||Boolean(schedule),busy||quizBusy);
  useEffect(()=>{if(!cover)return;let active=true;const reader=new FileReader();reader.onload=()=>{if(active)setLocalCover(String(reader.result));};reader.readAsDataURL(cover);return()=>{active=false;reader.abort();};},[cover]);

  function change<K extends keyof ArticleContent>(key:K,value:ArticleContent[K]){setContent(c=>{const next={...c,[key]:value};setDirty(JSON.stringify(next)!==baseline.current);return next;});setNotice('');}
  const archived=article?.status==='ARCHIVED';const errors=articleErrors(content);
  async function save(){
    setBusy(true);setError('');setNotice('');
    try{
      if(errors.length)throw new Error(errors.join(' '));
      let saved=await saveArticle(id,content,article?.version,quiz);setArticle(saved);setContent(editableArticle(saved));baseline.current=JSON.stringify(editableArticle(saved));setDirty(false);
      if(cover){
        try{saved=await uploadCover(id,saved.version,cover);setArticle(saved);setContent(editableArticle(saved));baseline.current=JSON.stringify(editableArticle(saved));setCover(null);}
        catch(e){setError(`Text draft saved. ${blogErrorMessage(e)}`);onSaved();return;}
      }
      setNotice('Draft saved. Preview remains private.');onSaved();
    }catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function act(action:string){
    if(!article||dirty||cover)return;
    if(action==='approve'&&article.can_approve!==true){setError(article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?independentApprovalMessage:'Approval permissions are unavailable. Reload the article.');return;}
    setBusy(true);setError('');setNotice('');
    try{const updated=await transitionArticle(article.id,article.version,action,action==='schedule'?nairobiSchedule(schedule):undefined);setArticle(updated);setContent(editableArticle(updated));setSchedule('');setNotice(`${action==='return'?'Returned to draft':updated.status.toLowerCase().replaceAll('_',' ')}. No announcement was sent.`);onSaved();}
    catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  /// Combined submit→approve→publish path for an Admin who doesn't need any
  /// other person's separate approval. Rather than predicting eligibility
  /// client-side (the backend's `can_approve` is only meaningful once a
  /// submission is already PENDING_REVIEW — it depends on the live
  /// `app.current_role`/`app.current_user_id` session context, not a
  /// static field this component could evaluate before submitting), this
  /// just attempts the full chain and lets the backend's own existing
  /// authorization at the `approve` step decide: an Admin who IS eligible
  /// ends with a published article in one click; one who is blocked
  /// (sensitive content they authored) correctly stops at PENDING_REVIEW
  /// with an honest notice, exactly the state a plain "Submit for review"
  /// click would have left them in — never a silent bypass.
  async function publishNow(){
    if(!article||dirty||cover)return;
    setBusy(true);setError('');setNotice('');
    try{
      let updated=await transitionArticle(article.id,article.version,'submit');
      try{
        updated=await transitionArticle(updated.id,updated.version,'approve');
        updated=await transitionArticle(updated.id,updated.version,'publish');
        setNotice('Published. No announcement was sent.');
      }catch{
        setNotice('Submitted for review, but approval or publication did not complete. Use Approve and Publish below.');
      }
      setArticle(updated);setContent(editableArticle(updated));onSaved();
    }catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}
  }
  async function reload(){setBusy(true);try{const updated=await getArticle(id);setArticle(updated);setContent(editableArticle(updated));baseline.current=JSON.stringify(editableArticle(updated));setDirty(false);setCover(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  function close(){requestNavigation(onClose);}
  return <div className="cms-editor">
    <div className="cms-editor-heading"><div><h2>{quiz?(article?'Edit quiz':'New quiz'):(article?'Edit article':'New article')}</h2><p>{article?.status.replaceAll('_',' ')||'DRAFT'} · Content version {article?.content_version||1}</p></div><button type="button" onClick={close}>{quiz?'Back to quizzes':'Back to articles'}</button></div>
    {error&&<div role="alert" className="cms-error">{error}{article&&<button type="button" onClick={reload}>Reload saved article</button>}{!article&&<button type="button" onClick={reload}>Recover saved draft</button>}</div>}
    {notice&&<p role="status" className="cms-notice">{notice}</p>}
    <div className="cms-editor-grid"><section className="cms-form">
      <fieldset disabled={busy||quizBusy||archived}><legend>{quiz?'Quiz details':'Article content'}</legend>
        <div className="cms-language" role="group" aria-label="Editor language"><button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English</button><button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili</button></div>
        <label>{language==='en'?'English':'Kiswahili'} title<input value={content[`title_${language}`]} maxLength={160} onChange={e=>change(`title_${language}`,e.target.value)}/></label>
        <label>{language==='en'?'English':'Kiswahili'} {quiz?'introduction':'content'}<textarea className="cms-body" value={content[`body_${language}`]} maxLength={30000} onChange={e=>change(`body_${language}`,e.target.value)} placeholder={quiz?'One or two sentences shown above the questions.':'## Heading\n\nWrite your article in Markdown…'}/></label>
        <p className="cms-hint">Markdown supports headings, **bold**, *italic*, lists, blockquotes and [links](https://example.org). HTML and embedded media are disabled.</p>
        <label>Category<select value={content.category_id} onChange={e=>change('category_id',e.target.value)}><option value="">Choose category</option>{categories.map(c=><option key={c.id} value={c.id}>{c.name_en} / {c.name_sw}</option>)}</select></label>
        {!quiz&&<><label>YouTube video link (optional)<input type="url" inputMode="url" value={content.video_url} maxLength={200} placeholder="https://www.youtube.com/watch?v=…" onChange={e=>change('video_url',e.target.value)}/></label>
        <p className="cms-hint">With a YouTube link this entry is a video: patients see it under Videos (in its category’s topic) and tapping it opens YouTube. Leave empty for a written article. The content above becomes the video’s description.</p></>}
        <label>Tags (comma separated)<input value={content.tags.join(',')} onChange={e=>change('tags',e.target.value?e.target.value.split(','):[])}/></label>
        <label className="cms-check"><input type="checkbox" checked={content.featured} onChange={e=>change('featured',e.target.checked)}/>Featured article</label>
        <label className="cms-check"><input type="checkbox" checked={content.sensitive} onChange={e=>change('sensitive',e.target.checked)}/>Sensitive health content</label>
        <p className="cms-hint">Mark medical, HIV prevention and adolescent-health content as sensitive. This labels the content only: you can submit, approve and publish your own work.</p>
        <label>Cover image (JPEG/PNG, up to 5 MB)<input type="file" accept="image/jpeg,image/png" onChange={e=>{const file=e.target.files?.[0]||null;if(file&&(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)){setError('Choose JPEG/PNG up to 5 MB.');return;}setCover(file);}}/></label>
        {cover&&<button type="button" onClick={()=>setCover(null)}>Remove selected cover</button>}
        <p className="cms-hint">Saving changes returns reviewed or published content to draft and clears its approval and schedule.</p>
        <button type="button" className="cms-primary" disabled={errors.length>0||(!dirty&&!cover)} onClick={save}>{busy?'Saving…':'Save draft'}</button>
        {errors.length>0&&<ul className="cms-hint">{errors.map(e=><li key={e}>{e}</li>)}</ul>}
      </fieldset>
      {quiz&&<p className="cms-hint">Save the quiz details, then add questions below and tick “Include MCQs in this article”. Patients see the quiz under HIV Prevention &amp; Sexual Health → Quizzes once both the quiz entry and its questions are published.</p>}
      <McqBuilder article={article} onPreview={setQuizPreview} onDirty={setQuizDirty} onPending={setQuizBusy}/>
      {article&&!archived&&<fieldset disabled={busy||quizBusy||dirty||Boolean(cover)}><legend>Review & publication</legend>
        {article.approved_at&&<p className="cms-hint">Approved content version {article.approved_version} · {new Date(article.approved_at).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT · Reviewer {article.approved_by}</p>}
        {article.scheduled_for&&<p>Scheduled: {new Date(article.scheduled_for).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT</p>}
        {article.status==='APPROVED'&&<label>Publication date · Africa/Nairobi (UTC+3)<input type="datetime-local" value={schedule} onChange={e=>setSchedule(e.target.value)}/></label>}
        <div className="cms-actions">{articleActions(article.status).map(action=><button type="button" key={action} disabled={action==='approve'&&article.can_approve!==true} title={action==='approve'&&article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?independentApprovalMessage:undefined} onClick={()=>{if(action!=='archive'||window.confirm('Archive this article? Archived articles cannot be edited.'))void act(action);}}>{({submit:'Submit for review',approve:'Approve',return:'Return for revision',publish:'Publish now',schedule:'Schedule publication',unpublish:'Return to draft / unpublish',archive:'Archive'} as Record<string,string>)[action]}</button>)}{article.status==='DRAFT'&&<button type="button" className="cms-primary" onClick={()=>void publishNow()}>Publish immediately</button>}</div>
        {(dirty||cover)&&<p>Save changes before review or publication.</p>}
      </fieldset>}
    </section><ArticlePreview content={content} category={categories.find(c=>c.id===content.category_id)} language={previewLanguage} setLanguage={setPreviewLanguage} article={article} localCover={cover?localCover:''} quiz={quizPreview}/></div>
  </div>;
}
