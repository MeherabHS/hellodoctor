import {request} from './api';
import type {QuizContent,QuizDocument,QuizRevision,ImportPreview} from './quizContent';
const base=(article:string)=>`/admin/articles/${encodeURIComponent(article)}/quiz`;
const expected=(revision:QuizRevision|null)=>({revision_id:revision?.id||null,version:revision?.version||null});
export const getQuiz=(article:string)=>request<QuizDocument>(base(article));
export const saveQuiz=(article:string,revision:QuizRevision|null,content:QuizContent)=>request<QuizDocument>(base(article),{method:'PATCH',body:JSON.stringify({...expected(revision),content})});
export const transitionQuiz=(article:string,revision:QuizRevision,action:string)=>request<QuizDocument>(`${base(article)}/${encodeURIComponent(revision.id)}/${action}`,{method:'POST',body:JSON.stringify({version:revision.version})});
export const previewImport=(article:string,file:{format:string;content:string})=>request<ImportPreview>(`${base(article)}/import-preview`,{method:'POST',body:JSON.stringify(file)});
export const confirmImport=(article:string,revision:QuizRevision|null,file:{format:string;content:string})=>request<QuizDocument>(`${base(article)}/import`,{method:'POST',body:JSON.stringify({...expected(revision),...file})});
