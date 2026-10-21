'use client';
import { useState } from 'react';
import { api, errorText } from '../lib/client';

export function ClinicalAttachment({conversation,message,size}:{conversation:string;message:string;size?:number}) {
  const [image,setImage]=useState('');
  const [busy,setBusy]=useState(false);
  const [error,setError]=useState('');
  async function view() {
    if(busy)return;
    setBusy(true);setError('');
    try {
      const value=await api<{mime_type:string;base64_content:string}>(`chat/${conversation}/messages/${message}/attachment`);
      if(!['image/jpeg','image/png'].includes(value.mime_type))throw new Error('Image format is unavailable.');
      setImage(`data:${value.mime_type};base64,${value.base64_content}`);
    } catch(e){setError(errorText(e));} finally{setBusy(false);}
  }
  return <div className="clinical-attachment"><strong>Clinical image</strong>{size!==undefined&&<span>{(size/1024).toFixed(1)} KiB</span>}<button type="button" className="dw-btn-outline-sm" disabled={busy} onClick={()=>void view()}>{busy?'Opening...':'View image'}</button>{error&&<p role="alert">{error}</p>}{image&&<div role="dialog" aria-modal="true" aria-label="Clinical image" className="clinical-image-dialog"><button type="button" autoFocus onClick={()=>setImage('')}>Close image</button><img src={image} alt="Clinical document shared in this consultation"/></div>}</div>;
}

export function PaperclipIcon(){return <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m21 11-9 9a6 6 0 0 1-8.5-8.5l10-10a4 4 0 0 1 5.7 5.7L9 17.4a2 2 0 0 1-2.8-2.8l9.2-9.2"/></svg>;}
