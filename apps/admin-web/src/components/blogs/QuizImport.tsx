'use client';
import {useEffect,useRef,useState} from 'react';
import {readQuizFile,quizTemplate,type QuizRevision,type QuizDocument,type ImportPreview} from '../../lib/quizContent';
import {previewImport,confirmImport} from '../../lib/quizApi';
import {blogErrorMessage} from '../../lib/blogErrors';
export function QuizImport({articleId,articleVersion,revision,disabled,onImported,begin,end,onDraftChange}:{articleId:string;revision:QuizRevision|null;disabled:boolean;onImported:(d:QuizDocument)=>void;articleVersion?:number;begin?:()=>boolean;end?:()=>void;onDraftChange?:(v:boolean)=>void}){
  const [file,setFile]=useState<{format:'csv'|'json';content:string}|null>(null),[preview,setPreview]=useState<ImportPreview|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  const active=useRef(true),locked=useRef(false),generation=useRef(0),input=useRef<HTMLInputElement>(null);
  useEffect(()=>{active.current=true;return()=>{active.current=false;};},[]);
  const source=`${articleId}:${articleVersion}:${revision?.id}:${revision?.version}`;
  const [previewSource,setPreviewSource]=useState('');
  const validPreview=previewSource===source?preview:null;
  useEffect(()=>{generation.current++;},[source]);
  function start(){if(locked.current||disabled||begin?.()===false)return null;locked.current=true;setBusy(true);return ++generation.current;}
  function finish(){locked.current=false;if(active.current)setBusy(false);end?.();}
  function current(ticket:number){return active.current&&generation.current===ticket;}
  function discard(){if(locked.current)return;generation.current++;setFile(null);setPreview(null);setError('');onDraftChange?.(false);if(input.current)input.current.value='';}
  function download(format:'csv'|'json'){const url=URL.createObjectURL(new Blob([quizTemplate(format)],{type:format==='csv'?'text/csv':'application/json'}));const a=document.createElement('a');a.href=url;a.download=`article-questions.${format}`;a.click();URL.revokeObjectURL(url);}
  async function select(selected:File|undefined){const ticket=start();if(ticket===null)return;setPreview(null);setFile(null);setError('');onDraftChange?.(Boolean(selected));try{if(selected){const value=await readQuizFile(selected);if(current(ticket))setFile(value);}}catch(e){if(current(ticket)){setError(blogErrorMessage(e));onDraftChange?.(false);}}finally{finish();}}
  async function review(){if(!file)return;const ticket=start();if(ticket===null)return;setError('');try{const value=await previewImport(articleId,file);if(current(ticket)){setPreview(value);setPreviewSource(source);}}catch(e){if(current(ticket))setError(blogErrorMessage(e));}finally{finish();}}
  async function confirm(){if(!file||!validPreview||validPreview.errors.length||!validPreview.questions.length)return;const ticket=start();if(ticket===null)return;setError('');try{const doc=await confirmImport(articleId,revision,file);if(current(ticket)){onImported(doc);setPreview(null);setFile(null);onDraftChange?.(false);if(input.current)input.current.value='';}}catch(e){if(current(ticket))setError(blogErrorMessage(e));}finally{finish();}}
  return <details className="cms-import"><summary>Import CSV / JSON questions</summary><p className="cms-hint">UTF-8, up to 128 KiB; 50 questions total. Import appends questions to the saved draft, preserves the inclusion setting and never publishes. All rows must pass; nothing is partially imported. Save manual changes first.</p>
    <div className="cms-actions"><button type="button" disabled={busy} onClick={()=>download('csv')}>CSV template</button><button type="button" disabled={busy} onClick={()=>download('json')}>JSON template</button></div>
    <label>Question import file<input ref={input} type="file" accept=".csv,.json,text/csv,application/json" disabled={disabled||busy} onChange={e=>void select(e.target.files?.[0])}/></label>
    <button type="button" disabled={disabled||busy||!file} onClick={review}>Preview import</button>
    {file&&<button type="button" disabled={busy} onClick={discard}>Discard import</button>}
    {error&&<p role="alert" className="cms-error">{error}</p>}
    {validPreview&&<div><h4>Import preview · {validPreview.questions.length} parsed questions</h4>{validPreview.errors.length>0?<ul role="alert">{validPreview.errors.map((e,i)=><li key={i}>Row {e.row||'file'} · {e.field}: {e.message}</li>)}</ul>:<p>Validation passed. Confirm to save an unpublished draft.</p>}<ol>{validPreview.questions.map(q=><li key={q.id}>{q.text_en} ({q.options.length} choices)</li>)}</ol><button type="button" disabled={disabled||busy||validPreview.errors.length>0||!validPreview.questions.length} onClick={confirm}>Confirm import to draft</button></div>}
  </details>;
}
