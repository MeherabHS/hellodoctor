'use client';
/* eslint-disable @next/next/no-img-element -- authenticated media route or a local data URL */
import {useEffect,useRef,useState} from 'react';
import {ImagePlus,Trash2} from 'lucide-react';
import {MAX_ADMIN_IMAGE_UPLOAD} from '../../lib/imageLimits';

/** Cover image for an article, video or quiz: shows the saved cover, or the
 *  newly chosen file before it is saved. */
export function CoverPicker({savedSrc,file,onFile,onError,note,label='Cover image'}:{savedSrc:string;file:File|null;onFile:(f:File|null)=>void;onError:(message:string)=>void;note?:string;label?:string}){
  const input=useRef<HTMLInputElement>(null);const [local,setLocal]=useState('');
  useEffect(()=>{
    if(!file){setLocal('');return;}
    let active=true;const reader=new FileReader();reader.onload=()=>{if(active)setLocal(String(reader.result));};reader.readAsDataURL(file);
    return()=>{active=false;reader.abort();};
  },[file]);
  const src=file?local:savedSrc;
  return <div>
    <span className="afya-field-label" style={{marginTop:0}}>{label}</span>
    <div className="cms-cover-picker">
      {src?<img src={src} alt="Cover" className="cms-cover-thumb"/>:<div className="cms-cover-thumb cms-cover-empty"><ImagePlus size={26} aria-hidden/></div>}
      <div style={{display:'flex',gap:8,alignItems:'center',flexWrap:'wrap'}}>
        <button type="button" className="afya-btn-secondary" onClick={()=>input.current?.click()}>{src?'Change image':'Upload image'}</button>
        {file&&<button type="button" className="afya-icon-btn" aria-label="Remove selected image" onClick={()=>onFile(null)}><Trash2 size={16} color="var(--afya-danger-fg)"/></button>}
      </div>
      <input ref={input} type="file" accept="image/jpeg,image/png" hidden aria-label={`${label} file`} onChange={e=>{
        const chosen=e.target.files?.[0]||null;e.target.value='';
        if(chosen&&(!['image/jpeg','image/png'].includes(chosen.type)||chosen.size>MAX_ADMIN_IMAGE_UPLOAD)){onError('Choose a JPEG or PNG up to 25 MB.');return;}
        onFile(chosen);
      }}/>
    </div>
    <span className="afya-stat-note">{note??'JPEG or PNG up to 25 MB. Compressed automatically when saved.'}</span>
  </div>;
}
