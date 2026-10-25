export type ArticleStatus = 'DRAFT'|'PENDING_REVIEW'|'APPROVED'|'SCHEDULED'|'PUBLISHED'|'ARCHIVED';
export interface ArticleContent {
  title_en:string; title_sw:string; body_en:string; body_sw:string;
  category_id:string; tags:string[]; featured:boolean; sensitive:boolean;
}
export interface AdminArticle extends ArticleContent {
  id:string; author_id:string; author_name:string; status:ArticleStatus;
  version:number; content_version:number; approved_version:number|null;
  approved_by:string|null; approved_at:string|null; requires_independent_review:boolean;
  can_approve:boolean; approval_block_reason:'INDEPENDENT_REVIEW_REQUIRED'|null;
  cover_image_id:string|null; created_at:string; updated_at:string; published_at:string|null;
  scheduled_for:string|null;
  category_name_en?:string; category_name_sw?:string;
}
export interface ArticleCategory { id:string; name_en:string; name_sw:string; sensitive:boolean }
export const articleStatuses:ArticleStatus[]=['DRAFT','PENDING_REVIEW','APPROVED','SCHEDULED','PUBLISHED','ARCHIVED'];
export const blankArticle=():ArticleContent=>({title_en:'',title_sw:'',body_en:'',body_sw:'',category_id:'',tags:[],featured:false,sensitive:true});
export function editableArticle(a:ArticleContent):ArticleContent {
  return {title_en:a.title_en,title_sw:a.title_sw,body_en:a.body_en,body_sw:a.body_sw,category_id:a.category_id,tags:[...a.tags],featured:a.featured,sensitive:a.sensitive};
}
export function articleErrors(c:ArticleContent):string[]{
  const errors:string[]=[];
  for(const [label,value,max,required] of [['English title',c.title_en,160,true],['English content',c.body_en,30000,true],['Kiswahili title',c.title_sw,160,false],['Kiswahili content',c.body_sw,30000,false]] as const){
    if((required&&!value.trim())||Array.from(value.trim()).length>max||value.includes('\0'))errors.push(`${label}: ${required?'1–':''}${max} characters required.`);
  }
  if(Boolean(c.title_sw.trim())!==Boolean(c.body_sw.trim()))errors.push('Provide both Kiswahili title and content, or leave both empty for English fallback.');
  if(!c.category_id)errors.push('Select a category.');
  if(c.tags.length>10||c.tags.some(t=>!t.trim()||Array.from(t.trim()).length>40||t.includes('\0')))errors.push('Use up to 10 tags of 1–40 characters.');
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
export function articleActions(status:ArticleStatus):string[]{
  switch(status){
    case 'DRAFT':return ['submit','archive'];
    case 'PENDING_REVIEW':return ['approve','return','archive'];
    case 'APPROVED':return ['publish','schedule','unpublish','archive'];
    case 'SCHEDULED':case 'PUBLISHED':return ['unpublish','archive'];
    default:return [];
  }
}
export function nairobiSchedule(value:string):string{
  if(!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value))throw new Error('Choose a publication date and time in Africa/Nairobi.');
  const date=new Date(`${value}:00+03:00`);
  if(!Number.isFinite(date.getTime())||date.getTime()<=Date.now())throw new Error('Publication must be scheduled in the future.');
  return date.toISOString();
}
