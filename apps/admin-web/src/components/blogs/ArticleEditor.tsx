'use client';
import {useRef,useState} from 'react';
import {ArrowLeft,Bold,Heading2,Italic,Link2,List,ListOrdered,Quote} from 'lucide-react';
import type {AdminArticle,ArticleContent,ArticleCategory} from '../../lib/blogContent';
import {articleErrors,blankArticle,editableArticle,nairobiSchedule} from '../../lib/blogContent';
import {getArticle,listCategories,saveCategory,transitionArticle} from '../../lib/blogApi';
import {categoryFor,choiceErrors,choiceFor,OTHER,topicOptions,type TopicChoice} from '../../lib/contentTopics';
import {CoverError,persistEntry,publishEntry} from '../../lib/entryPublish';
import {blogErrorMessage} from '../../lib/blogErrors';
import {isUnpublished} from '../../lib/publishFlow';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import {ArticlePreview} from './ArticlePreview';
import {CoverPicker} from './CoverPicker';
import {PublishCard} from './PublishCard';

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

/** Articles (Blog): a topic first (the six Patient app topics or "Other"),
 *  then a cover, title and Markdown text. Written articles only: videos and
 *  quizzes have their own sections. */
export function ArticleEditor({initial,categories:initialCategories,onSaved,onClose}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:(message?:string)=>void}){
  const [categories,setCategories]=useState(initialCategories);
  const [article,setArticle]=useState(initial);
  const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):blankArticle());
  const [choice,setChoice]=useState<TopicChoice>(()=>choiceFor(initialCategories.find(c=>c.id===initial?.category_id)));
  const [id]=useState(()=>initial?.id||crypto.randomUUID());
  const [language,setLanguage]=useState<'en'|'sw'>('en');const [previewLanguage,setPreviewLanguage]=useState<'en'|'sw'>('en');
  const [view,setView]=useState<'write'|'preview'>('write');
  const [cover,setCover]=useState<File|null>(null);
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const baseline=useRef(JSON.stringify({content,choice}));
  const bodyRef=useRef<HTMLTextAreaElement>(null);
  const textChanged=JSON.stringify({content,choice})!==baseline.current;
  const dirty=textChanged||Boolean(cover);
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty,busy);

  function change<K extends keyof ArticleContent>(key:K,value:ArticleContent[K]){setContent(c=>({...c,[key]:value}));setNotice('');}
  function settle(saved:AdminArticle){setArticle(saved);const next=editableArticle(saved);setContent(next);baseline.current=JSON.stringify({content:next,choice});}
  // Everything but the category is checked here; the topic is checked separately.
  const problems=[...choiceErrors(choice),...articleErrors({...content,category_id:content.category_id||'pending'})];
  const archived=article?.status==='ARCHIVED';

  /** Resolves the topic to a category, then saves text and cover. */
  async function persist():Promise<AdminArticle>{
    const resolved=await categoryFor(choice,categories,saveCategory);
    if(resolved.created)setCategories(await listCategories());
    const next={...content,category_id:resolved.id};
    const saved=await persistEntry(id,article,next,textChanged||next.category_id!==article?.category_id,cover);
    setCover(null);settle(saved);return saved;
  }
  async function run(work:()=>Promise<void>,failure=''){
    setBusy(true);setError('');setNotice('');
    try{await work();}
    catch(e){
      if(e instanceof CoverError){settle(e.saved);setError(`Text saved, but the image was not: ${blogErrorMessage(e.cause)}`);}
      else setError(`${failure}${blogErrorMessage(e)}`);
    }finally{setBusy(false);onSaved();}
  }
  const save=()=>run(async()=>{await persist();setNotice('Draft saved. Patients cannot see it until you publish.');});
  // Publish returns to the list: nothing is left unsaved, so no prompt.
  const publish=()=>run(async()=>{const saved=await persist();if(isUnpublished(saved.status))await publishEntry(saved);onClose(`Published “${saved.title_en}”.`);},'Not published: ');
  const schedule=(when:string)=>run(async()=>{const at=nairobiSchedule(when);const saved=await persist();await publishEntry(saved,'schedule',at);onClose(`Scheduled “${saved.title_en}”.`);});
  const action=(name:'unpublish'|'archive')=>run(async()=>{if(!article)return;settle(await transitionArticle(article.id,article.version,name));setNotice(name==='unpublish'?'Unpublished. It is a draft again.':'Archived.');});
  async function reload(){setBusy(true);try{settle(await getArticle(id));setCover(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  function format(f:Format){
    const area=bodyRef.current;if(!area)return;const next=applyFormat(area,f);change(`body_${language}`,next.value);
    requestAnimationFrame(()=>{area.focus();area.setSelectionRange(...next.cursor);});
  }
  const savedCover=article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'';
  const previewCategory=choice.topic===OTHER?{id:'',name_en:choice.other,name_sw:choice.other,sensitive:true,topic_key:null}:categories.find(c=>c.topic_key===choice.topic);

  return <div className="afya-stack">
    <div className="afya-page-head">
      <div style={{display:'flex',alignItems:'center',gap:12}}>
        <button type="button" className="afya-icon-btn" aria-label="Back to articles" onClick={()=>requestNavigation(()=>onClose())}><ArrowLeft size={20}/></button>
        <div><h2>{article?'Edit article':'New article'}</h2><p>{dirty?'Unsaved changes':article?'All changes saved':'Not saved yet'}</p></div>
      </div>
    </div>
    {error&&<div role="alert" className="afya-login-error">{error} {article&&<button type="button" className="afya-btn-secondary" onClick={reload}>Reload saved article</button>}</div>}
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}

    <div className="afya-two-col">
      <section className="admin-card" style={{padding:0}}>
        <div className="afya-tabs" role="tablist" aria-label="Editor view" style={{padding:'6px 22px 0',marginBottom:0}}>
          <button type="button" role="tab" aria-selected={view==='write'} className={`afya-tab ${view==='write'?'active':''}`} onClick={()=>setView('write')}>Write</button>
          <button type="button" role="tab" aria-selected={view==='preview'} className={`afya-tab ${view==='preview'?'active':''}`} onClick={()=>setView('preview')}>Patient preview</button>
        </div>
        {view==='write'?<fieldset disabled={busy||archived} className="afya-form" style={{border:0,margin:0,padding:'18px 22px 22px'}}>
          <label htmlFor="art-topic">Topic *</label>
          <select id="art-topic" className="afya-select" value={choice.topic} onChange={e=>{setChoice({topic:e.target.value as TopicChoice['topic'],other:choice.other});setNotice('');}}>
            <option value="">Choose a topic</option>
            {topicOptions.map(t=><option key={t.value} value={t.value}>{t.label}</option>)}
          </select>
          {choice.topic===OTHER&&<><label htmlFor="art-topic-other">Topic name *</label>
            <input id="art-topic-other" value={choice.other} maxLength={80} placeholder="e.g. Mental health" onChange={e=>setChoice({topic:OTHER,other:e.target.value})}/></>}

          <CoverPicker savedSrc={savedCover} file={cover} onFile={setCover} onError={setError}/>

          <div className="afya-lang-switch" role="group" aria-label="Editor language">
            <button type="button" aria-pressed={language==='en'} onClick={()=>setLanguage('en')}>English</button>
            <button type="button" aria-pressed={language==='sw'} onClick={()=>setLanguage('sw')}>Kiswahili (optional)</button>
          </div>
          <label htmlFor="art-title">Title{language==='en'?' *':''}</label>
          <input id="art-title" value={content[`title_${language}`]} maxLength={160} onChange={e=>change(`title_${language}`,e.target.value)}/>
          <label htmlFor="art-body">Article{language==='en'?' *':''}</label>
          <div className="afya-md-editor">
            <div className="afya-md-toolbar" role="toolbar" aria-label="Formatting">{FORMATS.map(f=>{const Icon=f.icon;return <button key={f.label} type="button" className="afya-icon-btn" title={f.label} aria-label={f.label} onClick={()=>format(f)}><Icon size={16}/></button>;})}</div>
            <textarea id="art-body" ref={bodyRef} className="afya-md-body" value={content[`body_${language}`]} maxLength={30000} onChange={e=>change(`body_${language}`,e.target.value)} placeholder={'## Heading\n\nWrite the article…'} rows={16}/>
          </div>
          <p className="afya-stat-note">{language==='sw'?'Leave Kiswahili empty and patients see the English text.':'Headings, bold, italic, lists, quotes and links.'}</p>
        </fieldset>:<div style={{padding:'8px 22px 22px'}}><ArticlePreview content={content} category={previewCategory} language={previewLanguage} setLanguage={setPreviewLanguage} article={article} localCover={''} quiz={null}/></div>}
      </section>

      <PublishCard article={article} noun="article" busy={busy} canSave={dirty||!article} problems={problems} onSave={save} onPublish={publish} onSchedule={schedule} onAction={action}/>
    </div>
  </div>;
}
