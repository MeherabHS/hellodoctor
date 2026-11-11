'use client';
import {useEffect,useRef,useState} from 'react';
import type {AdminArticle} from '../../lib/blogContent';
import {emptyQuiz,editableQuiz,newQuestion,quizErrors,quizRevision,type QuizContent,type QuizDocument,type QuizRevision,type QuizLanguage} from '../../lib/quizContent';
import {getQuiz,saveQuiz,transitionQuiz} from '../../lib/quizApi';
import {blogErrorMessage,independentApprovalMessage} from '../../lib/blogErrors';
import {QuestionEditor} from './QuestionEditor';
import {QuizImport} from './QuizImport';
export function McqBuilder({article,onPreview,onDirty,onPending}:{article:AdminArticle|null;onPreview:(c:QuizContent|null)=>void;onDirty:(d:boolean)=>void;onPending?:(v:boolean)=>void}){
  const [open,setOpen]=useState(false),[loaded,setLoaded]=useState(false),[revision,setRevision]=useState<QuizRevision|null>(null),[published,setPublished]=useState<QuizRevision|null>(null),[content,setContent]=useState<QuizContent>(emptyQuiz),[language,setLanguage]=useState<QuizLanguage>('en'),[dirty,setDirty]=useState(false),[busy,setBusy]=useState(false),[error,setError]=useState(''),[notice,setNotice]=useState('');
  const busyRef=useRef(false),alive=useRef(true),baseline=useRef(JSON.stringify(emptyQuiz()));const [importDraft,setImportDraft]=useState(false);
  useEffect(()=>{alive.current=true;return()=>{alive.current=false;};},[]);
  useEffect(()=>{onDirty(dirty||importDraft);},[dirty,importDraft,onDirty]);
  useEffect(()=>{onPending?.(busy);},[busy,onPending]);
  function begin(){if(busyRef.current)return false;busyRef.current=true;setBusy(true);return true;}
  function end(){busyRef.current=false;if(alive.current)setBusy(false);}
  function apply(doc:QuizDocument){
    if(!alive.current)return;
    if(doc.schema_version!==1)throw new Error('Unable to load the saved question contract.');
    const selected=quizRevision(doc);if(selected&&selected.article_id!==article?.id)throw new Error('Question response does not belong to this article.');
    const value=selected?editableQuiz(selected):emptyQuiz();setRevision(selected);setPublished(doc.published);setContent(value);baseline.current=JSON.stringify(value);setDirty(false);onPreview(value);setLoaded(true);
  }
  function change(value:QuizContent){if(busyRef.current)return;setContent(value);setDirty(JSON.stringify(value)!==baseline.current);onPreview(value);setNotice('');setError('');}
  async function load(){if(!article||!begin())return;setError('');try{apply(await getQuiz(article.id));}catch(e){if(alive.current)setError(blogErrorMessage(e));}finally{end();}}
  async function save(){if(!article||busyRef.current)return;const errors=quizErrors(content);if(errors.length){setError(errors.map(e=>`Question ${e.row}: ${e.message}`).join(' '));return;}if(!begin())return;setError('');try{const doc=await saveQuiz(article.id,revision,content);if(alive.current){apply(doc);setNotice('Question draft saved. Published article and quiz remain unchanged.');}}catch(e){if(alive.current)setError(blogErrorMessage(e));}finally{end();}}
  async function act(action:string){if(!article||!revision||dirty||busyRef.current)return;if(action==='approve'&&revision.can_approve!==true){setError(independentApprovalMessage);return;}if(!begin())return;setError('');try{const doc=await transitionQuiz(article.id,revision,action);if(alive.current){apply(doc);setNotice(action==='publish'?'Approved quiz setting published. No notification was sent.':'Question review status updated. Publication is a separate action.');}}catch(e){if(alive.current)setError(blogErrorMessage(e));}finally{end();}}
  const stale=Boolean(revision&&article&&revision.article_content_version!==article.content_version);
  const errors=quizErrors(content,content.include_quiz);
  return <fieldset className="cms-quiz" disabled={busy||article?.status==='ARCHIVED'}><legend>Article Questions / MCQ Builder</legend><p className="cms-hint">Optional. Draft questions stay private. Only an explicitly approved and published quiz setting changes what Patients receive.</p>
    {!article?<p>Save the article draft before adding questions. MCQs default to OFF.</p>:<button type="button" aria-expanded={open} onClick={()=>{if(busyRef.current)return;setOpen(!open);if(!loaded)void load();}}>{open?'Collapse MCQ Builder':'Open MCQ Builder'}</button>}
    {busy&&<p role="status">Editing paused while an editor operation is running.</p>}
    {error&&<p role="alert" className="cms-error">{error} <button type="button" onClick={load}>Reload saved questions</button></p>}
    {open&&loaded&&<><label className="cms-check"><input type="checkbox" checked={content.include_quiz} onChange={e=>change({...content,include_quiz:e.target.checked})}/>Include MCQs in this article</label>
      <p>{content.include_quiz?'Included draft quiz — approval and explicit publication required.':'Draft setting: article without MCQs. Questions are preserved; an existing published quiz changes only after approval and publication.'}</p>
      <p className="cms-hint">Current Patient quiz: {published?.include_quiz&&article?.status==='PUBLISHED'&&published.article_content_version===article.content_version?`${published.questions.length} questions`:'not included in the current published article'}. Saving questions never changes article visibility. Changes to the article body require the quiz to be saved and reviewed against the new article version.</p>
      <div className="cms-language" role="group" aria-label="Question editor language"><button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English questions</button><button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili questions</button></div>
      {content.questions.map((q,i)=><QuestionEditor key={q.id} question={q} index={i} total={content.questions.length} language={language} onChange={value=>change({...content,questions:content.questions.map(old=>old.id===value.id?value:old)})} onRemove={()=>change({...content,questions:content.questions.filter(old=>old.id!==q.id)})} onMove={offset=>{const questions=[...content.questions];[questions[i],questions[i+offset]]=[questions[i+offset],questions[i]];change({...content,questions});}}/>)}
      <div className="cms-actions"><button type="button" disabled={content.questions.length>=50} onClick={()=>change({...content,questions:[...content.questions,newQuestion()]})}>Add question</button><button type="button" className="cms-primary" onClick={save}>Save question draft</button></div>
      <p className="cms-hint">{content.questions.length}/50 questions · {revision?.status.replaceAll('_',' ')||'Unsaved draft'} · Plain text only; HTML is displayed as text. Complete Kiswahili per question or leave all translation fields blank for English fallback.</p>
      {content.include_quiz&&errors.length>0&&<ul className="cms-hint">{errors.map((e,i)=><li key={i}>Question {e.row||'quiz'} · {e.message}</li>)}</ul>}
      <QuizImport articleId={article!.id} articleVersion={article!.content_version} revision={revision} disabled={busy||dirty} begin={begin} end={end} onDraftChange={setImportDraft} onImported={doc=>{apply(doc);setNotice('Questions imported to draft. No article or quiz was published.');}}/>
      {stale&&<p role="status">Article content changed. Save the question draft again before review.</p>}
      {revision?.status==='PENDING_REVIEW'&&!revision.can_approve&&<p role="status">{independentApprovalMessage}</p>}
      <div className="cms-actions">{revision?.status==='DRAFT'&&<button type="button" disabled={dirty||stale||errors.length>0} onClick={()=>act('submit')}>Submit quiz for review</button>}{revision?.status==='PENDING_REVIEW'&&<button type="button" disabled={dirty||stale||!revision.can_approve} onClick={()=>act('approve')}>Approve quiz</button>}{revision&&['PENDING_REVIEW','APPROVED'].includes(revision.status)&&<button type="button" disabled={dirty||stale} onClick={()=>act('return')}>Return quiz for revision</button>}{revision?.status==='APPROVED'&&<button type="button" disabled={dirty||stale||article?.status!=='PUBLISHED'} onClick={()=>act('publish')}>Publish approved quiz setting</button>}</div>
      {revision?.status==='APPROVED'&&article?.status!=='PUBLISHED'&&<p>Publish the approved article first, then publish its approved quiz setting.</p>}
    </>}{notice&&<p role="status" className="cms-notice">{notice}</p>}
  </fieldset>;
}
