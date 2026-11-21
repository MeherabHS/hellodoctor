'use client';
import { tr } from '@/lib/i18n';
import {useEffect,useRef,useState} from 'react';
import {ArrowLeft,Plus} from 'lucide-react';
import type {AdminArticle,ArticleCategory,ArticleContent} from '../../lib/blogContent';
import {blankArticle,editableArticle} from '../../lib/blogContent';
import {getArticle,listCategories,saveCategory,transitionArticle} from '../../lib/blogApi';
import {getQuiz,saveQuiz,transitionQuiz} from '../../lib/quizApi';
import {maxQuestions,newQuestion,quizErrors,quizRevision,type QuizDocument,type QuizQuestion,type QuizRevision} from '../../lib/quizContent';
import {named,QUIZ_CATEGORY} from '../../lib/contentTopics';
import {CoverError,persistEntry,publishEntry} from '../../lib/entryPublish';
import {blogErrorMessage} from '../../lib/blogErrors';
import {isUnpublished,stepsBeforePublish} from '../../lib/publishFlow';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import {CoverPicker} from './CoverPicker';
import {PublishCard} from './PublishCard';
import {QuestionEditor} from './QuestionEditor';
import {QuizImport} from './QuizImport';

const copyQuestions=(q:QuizQuestion[])=>q.map(x=>({...x,options:x.options.map(o=>({...o}))}));

export function quizProblems(c:ArticleContent,questions:QuizQuestion[],complete:boolean):string[]{
  const problems:string[]=[];
  if(!c.title_en.trim())problems.push(tr('ui2.addAHeader'));
  if(!c.body_en.trim())problems.push(tr('ui2.addASubHeader'));
  if(Boolean(c.title_sw.trim())!==Boolean(c.body_sw.trim()))problems.push(tr('ui2.fillBothKiswahiliHeaderFieldsOr'));
  if(complete&&!questions.length)problems.push(tr('ui2.addAtLeastOneQuestion'));
  for(const e of quizErrors({include_quiz:complete,questions},complete))problems.push(e.row?tr('tpl.questionRowError', { row: e.row, message: e.message }):e.message);
  return [...new Set(problems)];
}

/** Quizzes: cover, header, sub header, then the questions. One Publish saves
 *  and publishes both the quiz entry and its questions. Patients find it
 *  under HIV Prevention & Sexual Health → Quizzes. */
export function QuizEditor({initial,categories,onSaved,onClose}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:(message?:string)=>void}){
  const [article,setArticle]=useState(initial);
  const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):{...blankArticle(),sensitive:false});
  const [id]=useState(()=>initial?.id||crypto.randomUUID());
  const [questions,setQuestions]=useState<QuizQuestion[]>(()=>initial?[]:[newQuestion()]);
  const [revision,setRevision]=useState<QuizRevision|null>(null);
  const [loaded,setLoaded]=useState(!initial);
  const [language,setLanguage]=useState<'en'|'sw'>('en');
  const [cover,setCover]=useState<File|null>(null);
  const [busy,setBusy]=useState(false);const [importing,setImporting]=useState(false);const [importDraft,setImportDraft]=useState(false);
  const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const textBaseline=useRef(JSON.stringify(content));const quizBaseline=useRef(JSON.stringify(questions));
  const textChanged=JSON.stringify(content)!==textBaseline.current;
  const quizChanged=JSON.stringify(questions)!==quizBaseline.current;
  const dirty=textChanged||quizChanged||Boolean(cover)||importDraft;
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty,busy||importing);

  function applyQuiz(doc:QuizDocument){
    const selected=quizRevision(doc);
    if(selected&&selected.article_id!==id)throw new Error(tr('ui2.theseQuestionsBelongToAnotherQuiz'));
    const next=selected?copyQuestions(selected.questions):[];
    setRevision(selected);setQuestions(next);quizBaseline.current=JSON.stringify(next);setLoaded(true);
  }
  useEffect(()=>{
    if(!initial)return;let active=true;
    getQuiz(initial.id).then(doc=>{if(active)applyQuiz(doc);}).catch(e=>{if(active)setError(blogErrorMessage(e));});
    return()=>{active=false;};
  // eslint-disable-next-line react-hooks/exhaustive-deps -- load once per opened quiz
  },[initial]);

  function change<K extends keyof ArticleContent>(key:K,value:ArticleContent[K]){setContent(c=>({...c,[key]:value}));setNotice('');}
  function setQuestionsLive(next:QuizQuestion[]){if(busy||importing)return;setQuestions(next);setNotice('');}
  function settle(saved:AdminArticle){setArticle(saved);const next=editableArticle(saved);setContent(next);textBaseline.current=JSON.stringify(next);}
  const archived=article?.status==='ARCHIVED';

  /** Saves the quiz entry, its cover and its questions. Questions are saved
   *  again whenever the entry changed, since they are tied to its version. */
  async function persist():Promise<{saved:AdminArticle;current:QuizRevision|null}>{
    let categoryId=content.category_id;
    if(!categoryId)categoryId=(await named(QUIZ_CATEGORY,categories.length?categories:await listCategories(),saveCategory)).id;
    const saved=await persistEntry(id,article,{...content,category_id:categoryId},textChanged||!article,cover,true);
    setCover(null);settle(saved);
    // The backend requires complete questions when the quiz is included, so a
    // half-written draft is saved without it; Publish needs it complete anyway.
    const include=questions.length>0&&quizErrors({include_quiz:true,questions},true).length===0;
    let current=revision;
    if(quizChanged||!current||current.article_content_version!==saved.content_version||current.include_quiz!==include){
      const doc=await saveQuiz(saved.id,current,{include_quiz:include,questions});applyQuiz(doc);current=quizRevision(doc);
    }
    return {saved,current};
  }
  async function run(work:()=>Promise<void>,failure=''){
    setBusy(true);setError('');setNotice('');
    try{await work();}
    catch(e){if(e instanceof CoverError){settle(e.saved);setError(tr('tpl.quizSavedImageNot', { error: blogErrorMessage(e.cause) }));}else setError(`${failure}${blogErrorMessage(e)}`);}
    finally{setBusy(false);onSaved();}
  }
  const save=()=>run(async()=>{await persist();setNotice(tr('ui.draftSavedPatientsCannotSeeIt'));});
  const publish=()=>run(async()=>{
    const {saved,current}=await persist();
    // The entry goes live first; its questions can only be published on a live entry.
    if(isUnpublished(saved.status))settle(await publishEntry(saved));
    let rev=current;
    if(rev&&isUnpublished(rev.status)){
      for(const step of stepsBeforePublish(rev.status)){const doc=await transitionQuiz(saved.id,rev,step);rev=doc.draft??rev;}
      await transitionQuiz(saved.id,rev,'publish');
    }
    onClose(tr('tpl.published', { title: saved.title_en }));
  },tr('tpl.notPublished'));
  const action=(name:'unpublish'|'archive')=>run(async()=>{if(!article)return;settle(await transitionArticle(article.id,article.version,name));setNotice(name==='unpublish'?tr('ui2.unpublishedItIsADraftAgain'):tr('ui2.archived'));});
  async function reload(){setBusy(true);try{settle(await getArticle(id));applyQuiz(await getQuiz(id));setCover(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  const savedCover=article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'';
  const live=Boolean(article&&!isUnpublished(article.status)&&!archived);
  const problems=quizProblems(content,questions,true);
  const sw=language==='sw';

  return <div className="afya-stack">
    <div className="afya-page-head">
      <div style={{display:'flex',alignItems:'center',gap:12}}>
        <button type="button" className="afya-icon-btn" aria-label={tr('ui.backToQuizzes')} onClick={()=>requestNavigation(()=>onClose())}><ArrowLeft size={20}/></button>
        <div><h2>{article?tr('ui2.editQuiz'):tr('ui2.newQuiz')}</h2><p>{dirty?tr('ui2.unsavedChanges'):article?tr('ui2.allChangesSaved'):tr('ui2.notSavedYet')}</p></div>
      </div>
    </div>
    {error&&<div role="alert" className="afya-login-error">{error} {article&&<button type="button" className="afya-btn-secondary" onClick={reload}>{tr('ui.reloadSavedQuiz')}</button>}</div>}
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}
    <div className="afya-two-col">
      <div className="afya-stack">
        <section className="admin-card">
          <fieldset disabled={busy||archived} className="afya-form" style={{border:0,margin:0,padding:0}}>
            <CoverPicker savedSrc={savedCover} file={cover} onFile={setCover} onError={setError}/>
            <div className="afya-lang-switch" role="group" aria-label={tr('ui.quizLanguage')}>
              <button type="button" aria-pressed={!sw} onClick={()=>setLanguage('en')}>{tr('ui.english')}</button>
              <button type="button" aria-pressed={sw} onClick={()=>setLanguage('sw')}>{tr('ui.kiswahiliOptional')}</button>
            </div>
            <label htmlFor="quiz-title">{tr('mix.header')}{sw?'':' *'}</label>
            <input id="quiz-title" value={content[`title_${language}`]} maxLength={160} placeholder={sw?'':'e.g. How much do you know about PrEP?'} onChange={e=>change(`title_${language}`,e.target.value)}/>
            <label htmlFor="quiz-sub">{tr('mix.subHeader')}{sw?'':' *'}</label>
            <textarea id="quiz-sub" value={content[`body_${language}`]} maxLength={600} rows={2} placeholder={sw?'':tr('ui2.oneSentenceShownAboveTheQuestions')} onChange={e=>change(`body_${language}`,e.target.value)}/>
            {sw&&<span className="afya-stat-note">{tr('ui.leaveKiswahiliEmptyAndPatientsSee')}</span>}
          </fieldset>
        </section>

        <section className="admin-card">
          <div className="cms-section-head">
            <div><h3>{tr('ui.questions')}</h3><p className="afya-stat-note">{questions.length}/{maxQuestions} · multiple choice · mark the correct answer for each</p></div>
          </div>
          {!loaded?<p className="afya-stat-note">{tr('ui.loadingQuestions')}</p>:<fieldset disabled={busy||importing||archived} style={{border:0,margin:0,padding:0}}>
            {questions.map((q,i)=><QuestionEditor key={q.id} question={q} index={i} total={questions.length} language={language}
              onChange={value=>setQuestionsLive(questions.map(old=>old.id===value.id?value:old))}
              onRemove={()=>setQuestionsLive(questions.filter(old=>old.id!==q.id))}
              onMove={offset=>{const next=[...questions];[next[i],next[i+offset]]=[next[i+offset],next[i]];setQuestionsLive(next);}}/>)}
            <button type="button" className="afya-btn-outline-green" disabled={questions.length>=maxQuestions} onClick={()=>setQuestionsLive([...questions,newQuestion()])}><Plus size={16} aria-hidden/> {tr('ui.addQuestion')}</button>
          </fieldset>}
          {article&&revision!==undefined&&<QuizImport articleId={article.id} articleVersion={article.content_version} revision={revision} disabled={busy||quizChanged||archived}
            begin={()=>{if(busy)return false;setImporting(true);return true;}} end={()=>setImporting(false)} onDraftChange={setImportDraft}
            onImported={doc=>{applyQuiz(doc);setNotice(tr('ui.questionsImportedIntoTheDraftPublish'));}}/>}
          {!article&&<p className="afya-stat-note" style={{marginTop:12}}>{tr('ui.importingQuestionsFromACsvOr')}</p>}
        </section>
      </div>
      <PublishCard article={article} noun="quiz" busy={busy||importing} canSave={dirty||!article} problems={problems}
        onSave={save} onPublish={publish} onAction={action}/>
    </div>
  </div>;
}
