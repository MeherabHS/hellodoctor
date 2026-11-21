'use client';
import { tr } from '@/lib/i18n';
import {useEffect,useRef,useState} from 'react';
import {ArrowLeft,ExternalLink} from 'lucide-react';
import type {AdminArticle,ArticleCategory,ArticleContent} from '../../lib/blogContent';
import {blankArticle,editableArticle,isYouTubeLink,nairobiSchedule} from '../../lib/blogContent';
import {getArticle,listCategories,saveCategory,transitionArticle} from '../../lib/blogApi';
import {named,VIDEO_CATEGORY,youTubeId} from '../../lib/contentTopics';
import {CoverError,persistEntry,publishEntry} from '../../lib/entryPublish';
import {blogErrorMessage} from '../../lib/blogErrors';
import {isUnpublished} from '../../lib/publishFlow';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import {CoverPicker} from './CoverPicker';
import {PublishCard} from './PublishCard';

/** The YouTube cover for a video id, as a file ready to upload as the cover. */
export async function fetchYouTubeCover(videoId:string):Promise<File>{
  const res=await fetch(`/api/youtube-thumbnail/${encodeURIComponent(videoId)}`,{credentials:'include',cache:'no-store'});
  if(!res.ok||res.headers.get('content-type')!=='image/jpeg')throw new Error(tr('ui2.noYoutubeCoverFoundUploadAn'));
  return new File([await res.blob()],`youtube-${videoId}.jpg`,{type:'image/jpeg'});
}

export function videoProblems(c:ArticleContent):string[]{
  const problems:string[]=[];
  if(!c.title_en.trim())problems.push(tr('ui2.addAHeader'));
  if(!c.body_en.trim())problems.push(tr('ui2.addASubHeader'));
  if(!c.video_url.trim())problems.push(tr('ui2.addTheYoutubeLink'));
  else if(!isYouTubeLink(c.video_url))problems.push(tr('ui2.theLinkMustBeAYoutube'));
  if(Boolean(c.title_sw.trim())!==Boolean(c.body_sw.trim()))problems.push(tr('ui2.fillBothKiswahiliFieldsOrLeave'));
  return problems;
}

/** Videos: cover (the YouTube cover, or an uploaded image), header, sub
 *  header and the YouTube link. Patients see it under Videos and tapping it
 *  opens YouTube. */
export function VideoEditor({initial,categories,onSaved,onClose}:{initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:(message?:string)=>void}){
  const [article,setArticle]=useState(initial);
  const [content,setContent]=useState<ArticleContent>(()=>initial?editableArticle(initial):{...blankArticle(),sensitive:false});
  const [id]=useState(()=>initial?.id||crypto.randomUUID());
  const [cover,setCover]=useState<File|null>(null);const [coverSource,setCoverSource]=useState<'youtube'|'upload'|null>(null);
  const [coverState,setCoverState]=useState<''|'loading'|'failed'>('');
  const [busy,setBusy]=useState(false);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const baseline=useRef(JSON.stringify(content));
  const textChanged=JSON.stringify(content)!==baseline.current;const dirty=textChanged||Boolean(cover);
  const {requestNavigation}=useAdminNavigation();useUnsavedChanges(dirty,busy);
  const videoId=youTubeId(content.video_url);

  // A new or changed link brings its YouTube cover once, unless the Admin
  // uploaded their own. Removing that cover does not fetch it again.
  const [fetchedFor,setFetchedFor]=useState<string|null>(()=>initial?.cover_image_id?youTubeId(initial.video_url??''):null);
  useEffect(()=>{
    if(!videoId||coverSource==='upload'||videoId===fetchedFor)return;
    let active=true;setCoverState('loading');
    fetchYouTubeCover(videoId).then(file=>{if(active){setCover(file);setCoverSource('youtube');setCoverState('');setFetchedFor(videoId);}}).catch(()=>{if(active){setCoverState('failed');setFetchedFor(videoId);}});
    return()=>{active=false;};
  },[videoId,coverSource,fetchedFor]);

  function change<K extends keyof ArticleContent>(key:K,value:ArticleContent[K]){setContent(c=>({...c,[key]:value}));setNotice('');}
  function settle(saved:AdminArticle){setArticle(saved);const next=editableArticle(saved);setContent(next);baseline.current=JSON.stringify(next);}
  const problems=videoProblems(content);const archived=article?.status==='ARCHIVED';

  async function persist():Promise<AdminArticle>{
    let categoryId=content.category_id;
    if(!categoryId){categoryId=(await named(VIDEO_CATEGORY,categories.length?categories:await listCategories(),saveCategory)).id;}
    const saved=await persistEntry(id,article,{...content,category_id:categoryId},textChanged||!article,cover);
    setCover(null);setCoverSource(null);settle(saved);return saved;
  }
  async function run(work:()=>Promise<void>,failure=''){
    setBusy(true);setError('');setNotice('');
    try{await work();}
    catch(e){if(e instanceof CoverError){settle(e.saved);setError(tr('tpl.videoSavedCoverNot', { error: blogErrorMessage(e.cause) }));}else setError(`${failure}${blogErrorMessage(e)}`);}
    finally{setBusy(false);onSaved();}
  }
  const save=()=>run(async()=>{await persist();setNotice(tr('ui.draftSavedPatientsCannotSeeIt'));});
  const publish=()=>run(async()=>{const saved=await persist();if(isUnpublished(saved.status))await publishEntry(saved);onClose(tr('tpl.published', { title: saved.title_en }));},tr('tpl.notPublished'));
  const schedule=(when:string)=>run(async()=>{const at=nairobiSchedule(when);const saved=await persist();await publishEntry(saved,'schedule',at);onClose(tr('tpl.scheduled', { title: saved.title_en }));});
  const action=(name:'unpublish'|'archive')=>run(async()=>{if(!article)return;settle(await transitionArticle(article.id,article.version,name));setNotice(name==='unpublish'?tr('ui2.unpublishedItIsADraftAgain'):tr('ui2.archived'));});
  async function reload(){setBusy(true);try{settle(await getArticle(id));setCover(null);setError('');}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  const savedCover=article?.cover_image_id?`/api/article-media/${article.id}?v=${article.version}`:'';

  return <div className="afya-stack">
    <div className="afya-page-head">
      <div style={{display:'flex',alignItems:'center',gap:12}}>
        <button type="button" className="afya-icon-btn" aria-label={tr('ui.backToVideos')} onClick={()=>requestNavigation(()=>onClose())}><ArrowLeft size={20}/></button>
        <div><h2>{article?tr('ui2.editVideo'):tr('ui2.newVideo')}</h2><p>{dirty?tr('ui2.unsavedChanges'):article?tr('ui2.allChangesSaved'):tr('ui2.notSavedYet')}</p></div>
      </div>
    </div>
    {error&&<div role="alert" className="afya-login-error">{error} {article&&<button type="button" className="afya-btn-secondary" onClick={reload}>{tr('ui.reloadSavedVideo')}</button>}</div>}
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}
    <div className="afya-two-col">
      <section className="admin-card">
        <fieldset disabled={busy||archived} className="afya-form" style={{border:0,margin:0,padding:0}}>
          <CoverPicker savedSrc={savedCover} file={cover} onFile={f=>{setCover(f);setCoverSource(f?'upload':null);}} onError={setError}
            note={coverState==='loading'?tr('ui2.fetchingTheYoutubeCover'):coverState==='failed'?tr('ui2.couldNotFetchTheYoutubeCover'):coverSource==='youtube'?tr('ui2.coverTakenFromYoutubeUploadAn'):tr('ui2.addedAutomaticallyFromTheYoutubeLink')}/>
          <label htmlFor="video-title">{tr('ui.header')}</label>
          <input id="video-title" value={content.title_en} maxLength={160} placeholder={tr('ui.eGHowPrepWorks')} onChange={e=>change('title_en',e.target.value)}/>
          <label htmlFor="video-sub">{tr('ui.subHeader')}</label>
          <textarea id="video-sub" value={content.body_en} maxLength={600} rows={3} placeholder={tr('ui.oneOrTwoSentencesAboutThe')} onChange={e=>change('body_en',e.target.value)}/>
          <label htmlFor="video-link">{tr('ui.youtubeLink')}</label>
          <div style={{display:'flex',gap:8}}>
            <input id="video-link" type="url" inputMode="url" value={content.video_url} maxLength={200} placeholder="https://www.youtube.com/watch?v=…" onChange={e=>change('video_url',e.target.value)} style={{flex:1}}/>
            {videoId&&<a className="afya-btn-secondary" href={`https://www.youtube.com/watch?v=${videoId}`} target="_blank" rel="noopener noreferrer"><ExternalLink size={14} aria-hidden/> {tr('ui.open')}</a>}
          </div>
          <details className="cms-later" style={{marginTop:14}} open={Boolean(content.title_sw||content.body_sw)}>
            <summary>{tr('ui.kiswahiliOptional')}</summary>
            <label htmlFor="video-title-sw">{tr('ui.headerKiswahili')}</label>
            <input id="video-title-sw" value={content.title_sw} maxLength={160} onChange={e=>change('title_sw',e.target.value)}/>
            <label htmlFor="video-sub-sw">{tr('ui.subHeaderKiswahili')}</label>
            <textarea id="video-sub-sw" value={content.body_sw} maxLength={600} rows={3} onChange={e=>change('body_sw',e.target.value)}/>
            <span className="afya-stat-note">{tr('ui.leaveEmptyAndPatientsSeeThe')}</span>
          </details>
        </fieldset>
      </section>
      <PublishCard article={article} noun="video" busy={busy||coverState==='loading'} canSave={dirty||!article} problems={problems} onSave={save} onPublish={publish} onSchedule={schedule} onAction={action}/>
    </div>
  </div>;
}
