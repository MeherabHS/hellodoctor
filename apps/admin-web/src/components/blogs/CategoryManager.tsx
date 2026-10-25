'use client';
import {useState} from 'react';
import type {ArticleCategory} from '../../lib/blogContent';
import {saveCategory,deleteCategory} from '../../lib/blogApi';
import {blogErrorMessage} from '../../lib/blogErrors';
export function CategoryManager({categories,reload}:{categories:ArticleCategory[];reload:()=>Promise<void>}){
  const [selected,setSelected]=useState<ArticleCategory|null>(null);const [en,setEn]=useState('');const [sw,setSw]=useState('');const [sensitive,setSensitive]=useState(true);const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  async function save(){setBusy(true);setError('');try{await saveCategory({name_en:en,name_sw:sw,sensitive},selected?.id);setSelected(null);setEn('');setSw('');setSensitive(true);await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  async function remove(id:string){if(!window.confirm('Delete this unused category?'))return;setBusy(true);try{await deleteCategory(id);await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  return <details className="cms-categories"><summary>Manage bilingual categories</summary>{error&&<p role="alert">{error}</p>}
    <div className="cms-category-form"><label>English name<input value={en} maxLength={80} onChange={e=>setEn(e.target.value)}/></label><label>Kiswahili name<input value={sw} maxLength={80} onChange={e=>setSw(e.target.value)}/></label><label className="cms-check"><input type="checkbox" disabled={Boolean(selected)} checked={sensitive} onChange={e=>setSensitive(e.target.checked)}/>Requires independent review</label><button type="button" disabled={busy||!en.trim()||!sw.trim()} onClick={save}>{selected?'Rename category':'Create category'}</button>{selected&&<button type="button" onClick={()=>{setSelected(null);setEn('');setSw('');setSensitive(true);}}>Cancel rename</button>}</div>
    <ul>{categories.map(c=><li key={c.id}>{c.name_en} / {c.name_sw}{c.sensitive?' · Independent review':''} <button type="button" disabled={busy} onClick={()=>{setSelected(c);setEn(c.name_en);setSw(c.name_sw);setSensitive(c.sensitive);}}>Rename</button> <button type="button" disabled={busy} onClick={()=>remove(c.id)}>Delete</button></li>)}</ul>
  </details>;
}
