'use client';
import { tr } from '@/lib/i18n';
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
  return <details className="cms-import"><summary>{tr('ui.importCsvJsonQuestions')}</summary><p className="cms-hint">{tr('ui.utf8UpTo128Kib')}</p>
    <div className="cms-actions"><button type="button" disabled={busy} onClick={()=>download('csv')}>{tr('ui.csvTemplate')}</button><button type="button" disabled={busy} onClick={()=>download('json')}>{tr('ui.jsonTemplate')}</button></div>
    <label>{tr('ui.questionImportFile')}<input ref={input} type="file" accept=".csv,.json,text/csv,application/json" disabled={disabled||busy} onChange={e=>void select(e.target.files?.[0])}/></label>
    <button type="button" disabled={disabled||busy||!file} onClick={review}>{tr('ui.previewImport')}</button>
    {file&&<button type="button" disabled={busy} onClick={discard}>{tr('ui.discardImport')}</button>}
    {error&&<p role="alert" className="cms-error">{error}</p>}
    {validPreview&&<div><h4>{tr('mix.importPreview', { count: validPreview.questions.length })}</h4>{validPreview.errors.length>0?<ul role="alert">{validPreview.errors.map((e,i)=><li key={i}>{tr('mix.importRow', { row: e.row||tr('mix.file'), field: e.field, message: e.message })}</li>)}</ul>:<p>{tr('ui.validationPassedConfirmToSaveAn')}</p>}<ol>{validPreview.questions.map(q=><li key={q.id}>{tr('mix.choicesCount', { text: q.text_en, count: q.options.length })}</li>)}</ol><button type="button" disabled={disabled||busy||validPreview.errors.length>0||!validPreview.questions.length} onClick={confirm}>{tr('ui.confirmImportToDraft')}</button></div>}
  </details>;
}
