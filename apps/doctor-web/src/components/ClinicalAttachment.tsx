'use client';
import { useState } from 'react';
import { api, errorText } from '../lib/client';
import { Icon } from './Icons';

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
  return <div className="am-attachment"><Icon name="image" size={18}/><span className="am-grow"><strong style={{display:'block',fontSize:13}}>Clinical image</strong>{size!==undefined&&<span style={{fontSize:11.5,opacity:.8}}>{(size/1024).toFixed(1)} KB</span>}</span><button type="button" className="am-btn ghost sm" disabled={busy} onClick={()=>void view()}>{busy?'Opening…':'View'}</button>{error&&<p role="alert">{error}</p>}{image&&<div role="dialog" aria-modal="true" aria-label="Clinical image" className="am-image-view"><img src={image} alt="Clinical image shared in this consultation"/><button type="button" className="am-btn primary" autoFocus onClick={()=>setImage('')}><Icon name="close" size={16}/>Close image</button></div>}</div>;
}
