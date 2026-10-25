import {request} from './api';
import type {AdminArticle,ArticleContent,ArticleCategory} from './blogContent';
export const listArticles=(query:URLSearchParams)=>request<{items:AdminArticle[];total:number}>(`/admin/articles?${query}`);
export const getArticle=(id:string)=>request<AdminArticle>(`/admin/articles/${encodeURIComponent(id)}`);
export const saveArticle=(id:string,content:ArticleContent,version?:number)=>request<AdminArticle>(`/admin/articles${version?`/${encodeURIComponent(id)}`:''}`,{method:version?'PATCH':'POST',body:JSON.stringify(version?{version,content}:{id,content})});
export const transitionArticle=(id:string,version:number,action:string,scheduled_for?:string)=>request<AdminArticle>(`/admin/articles/${encodeURIComponent(id)}/${action}`,{method:'POST',body:JSON.stringify({version,...(scheduled_for?{scheduled_for}:{})})});
export const listCategories=()=>request<ArticleCategory[]>('/admin/article-categories');
export const saveCategory=(category:Omit<ArticleCategory,'id'>,id?:string)=>request<ArticleCategory>(`/admin/article-categories${id?`/${encodeURIComponent(id)}`:''}`,{method:id?'PATCH':'POST',body:JSON.stringify(category)});
export const deleteCategory=(id:string)=>request(`/admin/article-categories/${encodeURIComponent(id)}`,{method:'DELETE'});
export async function uploadCover(id:string,version:number,file:File):Promise<AdminArticle>{
  if(!['image/jpeg','image/png'].includes(file.type)||file.size>5*1024*1024)throw new Error('Choose a JPEG or PNG up to 5 MB.');
  const res=await fetch(`/api/article-media/${encodeURIComponent(id)}?version=${version}`,{method:'POST',credentials:'include',headers:{'Content-Type':file.type},body:file});
  const data=await res.json();if(!res.ok||!data.success)throw new Error(data.error?.message||data.error||'Cover upload failed.');return data.data;
}
