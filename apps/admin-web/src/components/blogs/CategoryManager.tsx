'use client';
import {useState} from 'react';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import type {ArticleCategory,TopicKey} from '../../lib/blogContent';
import {educationTopics,topicLabel} from '../../lib/blogContent';
import {saveCategory,deleteCategory} from '../../lib/blogApi';
import {blogErrorMessage} from '../../lib/blogErrors';
export function CategoryManager({categories,reload}:{categories:ArticleCategory[];reload:()=>Promise<void>}){
  const [selected,setSelected]=useState<ArticleCategory|null>(null);const [en,setEn]=useState('');const [sw,setSw]=useState('');const [sensitive,setSensitive]=useState(true);const [topic,setTopic]=useState<TopicKey|''>('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  const {requestNavigation}=useAdminNavigation();
  useUnsavedChanges(selected?en!==selected.name_en||sw!==selected.name_sw||topic!==(selected.topic_key??''):Boolean(en||sw||!sensitive||topic),busy);
  function reset(){setSelected(null);setEn('');setSw('');setSensitive(true);setTopic('');}
  async function save(){setBusy(true);setError('');try{await saveCategory({name_en:en,name_sw:sw,sensitive,topic_key:topic||null},selected?.id);reset();await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  async function remove(id:string){if(!window.confirm('Delete this unused category?'))return;setBusy(true);try{await deleteCategory(id);await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  return <details className="cms-categories"><summary>Manage bilingual categories</summary>{error&&<p role="alert">{error}</p>}
    <fieldset className="cms-category-form" disabled={busy}><label>English name<input value={en} maxLength={80} onChange={e=>setEn(e.target.value)}/></label><label>Kiswahili name<input value={sw} maxLength={80} onChange={e=>setSw(e.target.value)}/></label>
      <label>Patient app topic<select value={topic} onChange={e=>setTopic(e.target.value as TopicKey|'')}><option value="">None (not listed under a topic)</option>{educationTopics.map(t=><option key={t.key} value={t.key}>{t.label}</option>)}</select></label>
      <label className="cms-check"><input type="checkbox" disabled={Boolean(selected)} checked={sensitive} onChange={e=>setSensitive(e.target.checked)}/>Sensitive health category</label><button type="button" disabled={busy||!en.trim()||!sw.trim()} onClick={save}>{selected?'Save category':'Create category'}</button>{selected&&<button type="button" onClick={reset}>Cancel edit</button>}</fieldset>
    <p className="cms-hint">Articles and videos in a category linked to a topic appear under that topic in HIV Prevention &amp; Sexual Health.</p>
    <ul>{categories.map(c=><li key={c.id}>{c.name_en} / {c.name_sw}{c.topic_key?` · Topic: ${topicLabel(c.topic_key)}`:''}{c.sensitive?' · Sensitive':''} <button type="button" disabled={busy} onClick={()=>requestNavigation(()=>{setSelected(c);setEn(c.name_en);setSw(c.name_sw);setSensitive(c.sensitive);setTopic(c.topic_key??'');})}>Edit</button> <button type="button" disabled={busy} onClick={()=>remove(c.id)}>Delete</button></li>)}</ul>
  </details>;
}
