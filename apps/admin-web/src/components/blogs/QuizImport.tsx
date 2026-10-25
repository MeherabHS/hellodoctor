'use client';
import {useState} from 'react';
import {readQuizFile,quizTemplate,type QuizRevision,type QuizDocument,type ImportPreview} from '../../lib/quizContent';
import {previewImport,confirmImport} from '../../lib/quizApi';
import {blogErrorMessage} from '../../lib/blogErrors';
export function QuizImport({articleId,revision,disabled,onImported}:{articleId:string;revision:QuizRevision|null;disabled:boolean;onImported:(d:QuizDocument)=>void}){
  const [file,setFile]=useState<{format:'csv'|'json';content:string}|null>(null),[preview,setPreview]=useState<ImportPreview|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  function download(format:'csv'|'json'){const url=URL.createObjectURL(new Blob([quizTemplate(format)],{type:format==='csv'?'text/csv':'application/json'}));const a=document.createElement('a');a.href=url;a.download=`article-questions.${format}`;a.click();URL.revokeObjectURL(url);}
  async function select(selected:File|undefined){setPreview(null);setFile(null);setError('');if(!selected)return;try{setFile(await readQuizFile(selected));}catch(e){setError(blogErrorMessage(e));}}
  async function review(){if(!file)return;setBusy(true);setError('');try{setPreview(await previewImport(articleId,file));}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  async function confirm(){if(!file||!preview||preview.errors.length||disabled)return;setBusy(true);setError('');try{onImported(await confirmImport(articleId,revision,file));setPreview(null);setFile(null);}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  return <details className="cms-import"><summary>Import CSV / JSON questions</summary><p className="cms-hint">UTF-8, up to 128 KiB; 50 questions total. Import appends questions to the saved draft, preserves the inclusion setting and never publishes. All rows must pass; nothing is partially imported. Save manual changes first.</p>
    <div className="cms-actions"><button type="button" onClick={()=>download('csv')}>CSV template</button><button type="button" onClick={()=>download('json')}>JSON template</button></div>
    <label>Question import file<input type="file" accept=".csv,.json,text/csv,application/json" disabled={disabled||busy} onChange={e=>void select(e.target.files?.[0])}/></label>
    <button type="button" disabled={disabled||busy||!file} onClick={review}>Preview import</button>
    {error&&<p role="alert" className="cms-error">{error}</p>}
    {preview&&<div><h4>Import preview · {preview.questions.length} parsed questions</h4>{preview.errors.length>0?<ul role="alert">{preview.errors.map((e,i)=><li key={i}>Row {e.row||'file'} · {e.field}: {e.message}</li>)}</ul>:<p>Validation passed. Confirm to save an unpublished draft.</p>}<ol>{preview.questions.map(q=><li key={q.id}>{q.text_en} ({q.options.length} choices)</li>)}</ol><button type="button" disabled={disabled||busy||preview.errors.length>0||!preview.questions.length} onClick={confirm}>Confirm import to draft</button></div>}
  </details>;
}
