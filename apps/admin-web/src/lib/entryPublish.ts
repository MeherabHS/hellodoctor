import type {AdminArticle,ArticleContent} from './blogContent';
import {saveArticle,transitionArticle,uploadCover} from './blogApi';
import {stepsBeforePublish} from './publishFlow';

/** Saves an article, video or quiz entry: the text when it changed (or is
 *  new), then a newly chosen cover. Returns the saved entry. A cover that
 *  fails to upload keeps the saved text and reports the cover separately. */
export async function persistEntry(id:string,article:AdminArticle|null,content:ArticleContent,changed:boolean,cover:File|null,standaloneQuiz=false):Promise<AdminArticle>{
  let saved=article;
  if(!saved||changed)saved=await saveArticle(id,content,saved?.version,standaloneQuiz);
  if(cover){
    try{saved=await uploadCover(id,saved.version,cover);}
    catch(e){throw new CoverError(saved,e);}
  }
  return saved;
}

export class CoverError extends Error{
  constructor(readonly saved:AdminArticle,readonly cause:unknown){super('Cover upload failed.');}
}

/** Publishes now, running the backend's remaining internal review steps
 *  first (Admins publish directly; lib/publishFlow). Already published = no-op. */
export async function publishEntry(article:AdminArticle,target:'publish'|'schedule'='publish',when?:string):Promise<AdminArticle>{
  if(article.status==='PUBLISHED'&&target==='publish')return article;
  let updated=article;
  for(const step of stepsBeforePublish(updated.status))updated=await transitionArticle(updated.id,updated.version,step);
  return transitionArticle(updated.id,updated.version,target,when);
}
