'use client';
import {useEffect,useRef,useState} from 'react';
import {ArrowLeft,Bold,Heading2,ImagePlus,Italic,Link2,List,ListOrdered,Quote,Trash2,X} from 'lucide-react';
import type {AdminArticle,ArticleContent,ArticleCategory} from '../../lib/blogContent';
import {articleActions,articleErrors,blankArticle,editableArticle,nairobiSchedule,topicLabel} from '../../lib/blogContent';
import {saveArticle,transitionArticle,uploadCover,getArticle} from '../../lib/blogApi';
import {ArticlePreview} from './ArticlePreview';
import {McqBuilder} from './McqBuilder';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import type {QuizContent} from '../../lib/quizContent';
import {blogErrorMessage,independentApprovalMessage} from '../../lib/blogErrors';
import {Card,Chip} from '../ui/afya';
import {STATUS_CHIP} from './statusChip';

const ACTION_LABEL:Record<string,string>={submit:'Submit for review',approve:'Approve',return:'Return for revision',publish:'Publish now',schedule:'Schedule publication',unpublish:'Return to draft / unpublish',archive:'Archive'};

/** Markdown formatting applied to the current selection of a textarea. */
type Format={label:string;icon:typeof Bold;wrap?:[string,string];prefix?:string;placeholder:string};
const FORMATS:Format[]=[
  {label:'Bold',icon:Bold,wrap:['**','**'],placeholder:'bold text'},
  {label:'Italic',icon:Italic,wrap:['*','*'],placeholder:'italic text'},
  {label:'Heading',icon:Heading2,prefix:'## ',placeholder:'Heading'},
  {label:'Bulleted list',icon:List,prefix:'- ',placeholder:'List item'},
  {label:'Numbered list',icon:ListOrdered,prefix:'1. ',placeholder:'List item'},
  {label:'Quote',icon:Quote,prefix:'> ',placeholder:'Quote'},
  {label:'Link',icon:Link2,wrap:['[','](https://)'],placeholder:'link text'},
];
function applyFormat(area:HTMLTextAreaElement,format:Format):{value:string;cursor:[number,number]}{
  const {value,selectionStart:s,selectionEnd:e}=area;const selected=value.slice(s,e)||format.placeholder;
  if(format.wrap){const [a,b]=format.wrap;return {value:value.slice(0,s)+a+selected+b+value.slice(e),cursor:[s+a.length,s+a.length+selected.length]};}
  const lineStart=value.lastIndexOf('\n',s-1)+1;const prefix=format.prefix??'';
  const block=(value.slice(s,e)||format.placeholder).split('\n').map(l=>prefix+l).join('\n');
  const before=value.slice(0,lineStart)+value.slice(lineStart,s);
  return {value:before+block+value.slice(e),cursor:[before.length,before.length+block.length]};
}

/** `mode='quiz'`: a standalone quiz (Quiz Manager). Same review and publish rules;
 * the content is a short introduction and there is no video link. */
export function ArticleEditor({initial,categories,onSaved,onClose,mode='article'}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:()=>void;mode?:'article'|'quiz'}){
  const quiz=mode==='quiz'||initial?.standalone_quiz===true;
  const [article,setArticle]=useState(initial);const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):blankArticle());
  const [id]=useState(()=>initial?.id||crypto.randomUUID());const [language,setLanguage]=useState<'en'|'sw'>('en');const [previewLanguage,setPreviewLanguage]=useState<'en'|'sw'>('en');
  const [view,setView]=useState<'write'|'preview'>('write');const [tagDraft,setTagDraft]=useState('');
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');const [dirty,setDirty]=useState(false);
  const [schedule,setSchedule]=useState('');const [cover,setCover]=useState<File|null>(null);const [localCover,setLocalCover]=useState('');
  const [quizPreview,setQuizPreview]=useState<QuizContent|null>(null);const [quizDirty,setQuizDirty]=useState(false);
  const [quizBusy,setQuizBusy]=useState(false);const baseline=useRef(JSON.stringify(initial?editableArticle(initial):blankArticle()));
  const bodyRef=useRef<HTMLTextAreaElement>(null);const fileRef=useRef<HTMLInputElement>(null);
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty||Boolean(cover)||quizDirty||Boolean(schedule)||Boolean(tagDraft.trim()),busy||quizBusy);
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
  /// Combined submit→approve→publish for one Admin (single-Admin approval,
  /// CLAUDE.md rule 7). The backend authorizes each step; if approval or
  /// publication does not complete, the entry honestly stays in review.
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
  function format(f:Format){
    const area=bodyRef.current;if(!area)return;const next=applyFormat(area,f);change(`body_${language}`,next.value);
    requestAnimationFrame(()=>{area.focus();area.setSelectionRange(...next.cursor);});
  }
  function addTags(raw:string){
    const incoming=raw.split(',').map(t=>t.trim()).filter(Boolean);if(!incoming.length)return;
    change('tags',[...content.tags,...incoming.filter(t=>!content.tags.includes(t))]);setTagDraft('');
  }
  const status=article?STATUS_CHIP[article.status]:STATUS_CHIP.DRAFT;
  const coverSrc=cover?localCover:(article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'');
  const locked=busy||quizBusy||archived;

  return <div className="afya-stack">
    <div className="afya-page-head">
      <div style={{display:'flex',alignItems:'center',gap:12}}>
        <button type="button" className="afya-icon-btn" aria-label={quiz?'Back to quizzes':'Back to articles'} onClick={close}><ArrowLeft size={20}/></button>
        <div><h2>{quiz?(article?'Edit Quiz':'Create Quiz'):(article?'Edit Article':'Create Article')}</h2>
          <p style={{display:'flex',gap:8,alignItems:'center'}}><Chip tone={status.tone}>{status.label}</Chip>Content version {article?.content_version||1}{dirty||cover?' · Unsaved changes':''}</p></div>
      </div>
    </div>
    {error&&<div role="alert" className="afya-login-error">{error} <button type="button" className="afya-btn-secondary" onClick={reload}>{article?'Reload saved article':'Recover saved draft'}</button></div>}
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}

    <div className="afya-two-col">
      <div className="afya-stack">
        <section className="admin-card" style={{padding:0}}>
          <div className="afya-tabs" role="tablist" aria-label="Editor view" style={{padding:'6px 22px 0',marginBottom:0}}>
            <button type="button" role="tab" aria-selected={view==='write'} className={`afya-tab ${view==='write'?'active':''}`} onClick={()=>setView('write')}>Write</button>
            <button type="button" role="tab" aria-selected={view==='preview'} className={`afya-tab ${view==='preview'?'active':''}`} onClick={()=>setView('preview')}>Patient preview</button>
          </div>
          {view==='write'?<fieldset disabled={locked} className="afya-form" style={{border:0,margin:0,padding:'18px 22px 22px'}}>
            <div className="afya-lang-switch" role="group" aria-label="Editor language">
              <button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English</button>
              <button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili</button>
            </div>
            <label htmlFor="art-title">Title ({language==='en'?'English':'Kiswahili'}){language==='en'&&' *'}</label>
            <input id="art-title" value={content[`title_${language}`]} maxLength={160} onChange={e=>change(`title_${language}`,e.target.value)}/>
            <label htmlFor="art-body">{quiz?'Introduction':'Content'} ({language==='en'?'English':'Kiswahili'}){language==='en'&&' *'}</label>
            <div className="afya-md-editor">
              {!quiz&&<div className="afya-md-toolbar" role="toolbar" aria-label="Formatting">{FORMATS.map(f=>{const Icon=f.icon;return <button key={f.label} type="button" className="afya-icon-btn" title={f.label} aria-label={f.label} onClick={()=>format(f)}><Icon size={16}/></button>;})}</div>}
              <textarea id="art-body" ref={bodyRef} className="afya-md-body" value={content[`body_${language}`]} maxLength={30000} onChange={e=>change(`body_${language}`,e.target.value)} placeholder={quiz?'One or two sentences shown above the questions.':'## Heading\n\nWrite your article in Markdown…'} rows={quiz?5:16}/>
            </div>
            <p className="afya-stat-note">Markdown: headings, **bold**, *italic*, lists, quotes and links. HTML and embedded media are disabled. Kiswahili is optional; without it patients see the English text.</p>
            {!quiz&&<><label htmlFor="art-video">YouTube video link (optional)</label>
              <input id="art-video" type="url" inputMode="url" value={content.video_url} maxLength={200} placeholder="https://www.youtube.com/watch?v=…" onChange={e=>change('video_url',e.target.value)}/>
              <p className="afya-stat-note">With a YouTube link this entry is a video: patients see it under Videos in its topic and tapping it opens YouTube. The content above becomes its description.</p></>}
          </fieldset>:<div style={{padding:'8px 22px 22px'}}><ArticlePreview content={content} category={categories.find(c=>c.id===content.category_id)} language={previewLanguage} setLanguage={setPreviewLanguage} article={article} localCover={cover?localCover:''} quiz={quizPreview}/></div>}
        </section>
        {quiz&&<p className="afya-stat-note">Save the quiz details, then add questions below and tick “Include MCQs in this article”. Patients see the quiz under HIV Prevention &amp; Sexual Health → Quizzes once both the quiz entry and its questions are published.</p>}
        <div className="cms-manager" style={{padding:0,maxWidth:'none'}}><McqBuilder article={article} onPreview={setQuizPreview} onDirty={setQuizDirty} onPending={setQuizBusy}/></div>
      </div>

      <div className="afya-stack">
        <Card title="Details">
          <fieldset disabled={locked} className="afya-form" style={{border:0,padding:0,margin:0}}>
            <span className="afya-field-label" style={{marginTop:0}}>Cover image</span>
            <div style={{display:'flex',gap:12,alignItems:'center'}}>
              {/* eslint-disable-next-line @next/next/no-img-element -- authenticated media route or local data URL */}
              {coverSrc?<img src={coverSrc} alt="Cover" style={{width:120,height:76,objectFit:'cover',borderRadius:10,background:'var(--afya-line-soft)'}}/>:<div style={{width:120,height:76,borderRadius:10,display:'grid',placeItems:'center',background:'var(--afya-green-soft)',color:'var(--afya-green)'}}><ImagePlus size={24} aria-hidden/></div>}
              <div style={{display:'flex',gap:8}}>
                <button type="button" className="afya-btn-secondary" onClick={()=>fileRef.current?.click()}>{coverSrc?'Change':'Upload'}</button>
                {cover&&<button type="button" className="afya-icon-btn" aria-label="Remove selected cover" onClick={()=>setCover(null)}><Trash2 size={16} color="var(--afya-danger-fg)"/></button>}
              </div>
              <input ref={fileRef} type="file" accept="image/jpeg,image/png" hidden aria-label="Cover image file" onChange={e=>{const file=e.target.files?.[0]||null;e.target.value='';if(file&&(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)){setError('Choose JPEG/PNG up to 5 MB.');return;}setCover(file);}}/>
            </div>
            <span className="afya-stat-note">JPEG or PNG, up to 5 MB.</span>

            <label htmlFor="art-cat">Topic / Category *</label>
            <select id="art-cat" value={content.category_id} onChange={e=>change('category_id',e.target.value)}><option value="">Choose category</option>{categories.map(c=><option key={c.id} value={c.id}>{c.topic_key?`${topicLabel(c.topic_key)} · `:''}{c.name_en} / {c.name_sw}</option>)}</select>

            <label htmlFor="art-tags">Tags</label>
            <div className="afya-tag-input">
              {content.tags.map(t=><span key={t} className="afya-chip topic">{t}<button type="button" aria-label={`Remove tag ${t}`} onClick={()=>change('tags',content.tags.filter(x=>x!==t))}><X size={12}/></button></span>)}
              <input id="art-tags" value={tagDraft} placeholder={content.tags.length?'':'Add a tag and press Enter'} onChange={e=>{const v=e.target.value;if(v.includes(','))addTags(v);else setTagDraft(v);}} onKeyDown={e=>{if(e.key==='Enter'){e.preventDefault();addTags(tagDraft);}else if(e.key==='Backspace'&&!tagDraft&&content.tags.length)change('tags',content.tags.slice(0,-1));}} onBlur={()=>addTags(tagDraft)}/>
            </div>

            <label className="afya-switch-row"><span>Featured {quiz?'quiz':'article'}</span><input type="checkbox" role="switch" checked={content.featured} onChange={e=>change('featured',e.target.checked)}/></label>
            <label className="afya-switch-row"><span>Sensitive health content</span><input type="checkbox" role="switch" checked={content.sensitive} onChange={e=>change('sensitive',e.target.checked)}/></label>
            <span className="afya-stat-note">A content label only: you can submit, approve and publish your own work.</span>

            {errors.length>0&&<ul className="afya-stat-note" style={{margin:'10px 0 0',paddingLeft:18}}>{errors.map(e=><li key={e}>{e}</li>)}</ul>}
            <div style={{display:'flex',gap:10,marginTop:14}}>
              <button type="button" className="afya-btn-outline-green" style={{flex:1}} disabled={errors.length>0||(!dirty&&!cover)} onClick={save}>{busy?'Saving…':'Save draft'}</button>
              {article?.status==='DRAFT'&&<button type="button" className="afya-btn-primary" style={{flex:1}} disabled={dirty||Boolean(cover)} onClick={()=>void publishNow()}>Publish</button>}
            </div>
            <span className="afya-stat-note">Saving changes returns reviewed or published content to draft and clears its approval and schedule.</span>
          </fieldset>
        </Card>

        {article&&!archived&&<Card title="Review & publication">
          <fieldset disabled={busy||quizBusy||dirty||Boolean(cover)} className="afya-form" style={{border:0,padding:0,margin:0}}>
            {article.approved_at&&<span className="afya-stat-note">Approved content version {article.approved_version} · {new Date(article.approved_at).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT</span>}
            {article.scheduled_for&&<span className="afya-stat-note">Scheduled: {new Date(article.scheduled_for).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT</span>}
            {article.status==='APPROVED'&&<><label htmlFor="art-schedule">Publication date · Africa/Nairobi (UTC+3)</label><input id="art-schedule" type="datetime-local" value={schedule} onChange={e=>setSchedule(e.target.value)}/></>}
            <div style={{display:'flex',gap:8,flexWrap:'wrap',marginTop:6}}>
              {articleActions(article.status).map(action=><button type="button" key={action} className={action==='archive'?'afya-btn-danger':action==='publish'||action==='approve'?'afya-btn-primary':'afya-btn-secondary'} disabled={action==='approve'&&article.can_approve!==true} title={action==='approve'&&article.approval_block_reason==='INDEPENDENT_REVIEW_REQUIRED'?independentApprovalMessage:undefined} onClick={()=>{if(action!=='archive'||window.confirm('Archive this article? Archived articles cannot be edited.'))void act(action);}}>{ACTION_LABEL[action]}</button>)}
            </div>
            {(dirty||cover)&&<span className="afya-stat-note">Save changes before review or publication.</span>}
          </fieldset>
        </Card>}
      </div>
    </div>
  </div>;
}
