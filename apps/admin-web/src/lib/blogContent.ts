import { currentLocale, tr } from './i18n/core.ts';
export type ArticleStatus = 'DRAFT'|'PENDING_REVIEW'|'APPROVED'|'SCHEDULED'|'PUBLISHED'|'ARCHIVED';
export interface ArticleContent {
  title_en:string; title_sw:string; body_en:string; body_sw:string;
  category_id:string; tags:string[]; featured:boolean; sensitive:boolean;
  /** YouTube link; a non-empty value makes this entry a video in the Patient app. '' = written article. */
  video_url:string;
}
export interface AdminArticle extends ArticleContent {
  id:string; author_id:string; author_name:string; status:ArticleStatus;
  version:number; content_version:number; approved_version:number|null;
  approved_by:string|null; approved_at:string|null; requires_independent_review:boolean;
  can_approve:boolean; approval_block_reason:'INDEPENDENT_REVIEW_REQUIRED'|null;
  cover_image_id:string|null; created_at:string; updated_at:string; published_at:string|null;
  scheduled_for:string|null;
  category_name_en?:string; category_name_sw?:string;
  /** A quiz that is not attached to a blog article (Quiz Manager). */
  standalone_quiz?:boolean;
}
export type TopicKey='HIV_BASICS'|'PREP'|'CONDOMS'|'SRH'|'STIGMA_DISCLOSURE'|'HEALTHY_RELATIONSHIPS';
/** The six HIV Prevention & Sexual Health topics shown in the Patient app. */
export const educationTopics:{key:TopicKey;label:string}[]=[
  {key:'HIV_BASICS',label:'HIV Basics'},{key:'PREP',label:'PrEP'},{key:'CONDOMS',label:'Condoms'},
  {key:'SRH',label:'Sexual & Reproductive Health'},{key:'STIGMA_DISCLOSURE',label:'Stigma & Disclosure'},
  {key:'HEALTHY_RELATIONSHIPS',label:'Healthy Relationships'},
];
/** Kiswahili names, as the Patient app shows them (app_sw.arb eduTopic*). */
export const TOPIC_SW:Record<TopicKey,string>={
  HIV_BASICS:'Msingi wa VVU',PREP:'PrEP',CONDOMS:'Kondomu',SRH:'Afya ya Uzazi na Ujinsia',
  STIGMA_DISCLOSURE:'Unyanyapaa na Kujiweka Wazi',HEALTHY_RELATIONSHIPS:'Mahusiano Yenye Afya',
};
/** A topic's name in the Admin's language (the English label is also category data). */
export const topicLabel=(key:string|null|undefined)=>{
  const topic=educationTopics.find(t=>t.key===key);
  if(!topic)return '';
  return currentLocale()==='sw'?TOPIC_SW[topic.key]:topic.label;
};
export interface ArticleCategory { id:string; name_en:string; name_sw:string; sensitive:boolean; topic_key:TopicKey|null }
/** Mirrors the backend's accepted YouTube link shapes (it stores the canonical watch URL). */
export function isYouTubeLink(value:string):boolean{
  return /^https?:\/\/((www\.|m\.)?youtube\.com\/(watch\?([^#]*&)?v=|shorts\/|embed\/|live\/)|youtu\.be\/)[A-Za-z0-9_-]{11}([?&#/].*)?$/i.test(value.trim());
}
export const articleStatuses:ArticleStatus[]=['DRAFT','PENDING_REVIEW','APPROVED','SCHEDULED','PUBLISHED','ARCHIVED'];
export const blankArticle=():ArticleContent=>({title_en:'',title_sw:'',body_en:'',body_sw:'',category_id:'',tags:[],featured:false,sensitive:true,video_url:''});
export function editableArticle(a:ArticleContent):ArticleContent {
  return {title_en:a.title_en,title_sw:a.title_sw,body_en:a.body_en,body_sw:a.body_sw,category_id:a.category_id,tags:[...a.tags],featured:a.featured,sensitive:a.sensitive,video_url:a.video_url??''};
}
export function articleErrors(c:ArticleContent):string[]{
  const errors:string[]=[];
  for(const [label,value,max,required] of [[tr('lib.englishTitle'),c.title_en,160,true],[tr('lib.englishContent'),c.body_en,30000,true],[tr('lib.kiswahiliTitle'),c.title_sw,160,false],[tr('lib.kiswahiliContent'),c.body_sw,30000,false]] as const){
    if((required&&!value.trim())||Array.from(value.trim()).length>max||value.includes('\0'))errors.push(tr('lib.charactersRequired', { label, range: `${required?'1–':''}${max}` }));
  }
  if(Boolean(c.title_sw.trim())!==Boolean(c.body_sw.trim()))errors.push(tr('lib.provideBothSw'));
  if(!c.category_id)errors.push(tr('lib.selectCategory'));
  if(c.video_url.trim()&&!isYouTubeLink(c.video_url))errors.push(tr('lib.videoLinkYouTube'));
  if(c.tags.length>10||c.tags.some(t=>!t.trim()||Array.from(t.trim()).length>40||t.includes('\0')))errors.push(tr('lib.tagsLimit'));
  return errors;
}
export function safeArticleLink(value:string):string {
  if(/[\u0000-\u0020\u007f\\]/.test(value))return '';
  try{const url=new URL(value);return ['https:','http:'].includes(url.protocol)&&url.hostname&&!url.username&&!url.password?url.href:'';}catch{return '';}
}
export function articleLanguage(c:ArticleContent,language:'en'|'sw'){
  const sw=language==='sw'&&Boolean(c.title_sw.trim()&&c.body_sw.trim());
  return {title:sw?c.title_sw:c.title_en,body:sw?c.body_sw:c.body_en,fallback:language==='sw'&&!sw};
}
/** Secondary actions in the Publication card. Publish and Schedule are their
 *  own controls; there is no review step in the Admin UI (lib/publishFlow). */
export function articleActions(status:ArticleStatus):string[]{
  switch(status){
    case 'DRAFT':case 'PENDING_REVIEW':case 'APPROVED':return ['archive'];
    case 'SCHEDULED':case 'PUBLISHED':return ['unpublish','archive'];
    default:return [];
  }
}
export function nairobiSchedule(value:string):string{
  if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value))throw new Error(tr('lib.choosePublicationTime'));
  const date=new Date(`${value}:00+03:00`);
  if(!Number.isFinite(date.getTime())||date.getTime()<=Date.now())throw new Error(tr('lib.publicationFuture'));
  return date.toISOString();
}
