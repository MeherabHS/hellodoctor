'use client';
import {useEffect,useState} from 'react';
import {Pencil,Trash2} from 'lucide-react';
import {useAdminNavigation,useUnsavedChanges} from '../navigation/NavigationGuard';
import type {ArticleCategory,TopicKey} from '../../lib/blogContent';
import {educationTopics,topicLabel} from '../../lib/blogContent';
import {saveCategory,deleteCategory,listCategories} from '../../lib/blogApi';
import {blogErrorMessage} from '../../lib/blogErrors';
import {Card,Chip,EmptyState,PageHead} from '../ui/afya';

/** Category form and list. Same create/edit/delete rules as before; only the layout changed. */
export function CategoryManager({categories,reload}:{categories:ArticleCategory[];reload:()=>Promise<void>}){
  const [selected,setSelected]=useState<ArticleCategory|null>(null);const [en,setEn]=useState('');const [sw,setSw]=useState('');const [sensitive,setSensitive]=useState(true);const [topic,setTopic]=useState<TopicKey|''>('');const [busy,setBusy]=useState(false);const [error,setError]=useState('');
  const {requestNavigation}=useAdminNavigation();
  useUnsavedChanges(selected?en!==selected.name_en||sw!==selected.name_sw||topic!==(selected.topic_key??''):Boolean(en||sw||!sensitive||topic),busy);
  function reset(){setSelected(null);setEn('');setSw('');setSensitive(true);setTopic('');}
  async function save(){setBusy(true);setError('');try{await saveCategory({name_en:en,name_sw:sw,sensitive,topic_key:topic||null},selected?.id);reset();await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  async function remove(id:string){if(!window.confirm('Delete this unused category?'))return;setBusy(true);try{await deleteCategory(id);await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  return <div className="afya-two-col">
    <Card title="Categories" subtitle="Articles and videos in a category linked to a topic appear under that topic in HIV Prevention & Sexual Health.">
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error}</div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Category</th><th>Patient app topic</th><th>Label</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{categories.map(c=><tr key={c.id}>
            <td><div style={{fontWeight:700}}>{c.name_en}</div><div className="afya-stat-note">{c.name_sw}</div></td>
            <td>{c.topic_key?<Chip tone="topic">{topicLabel(c.topic_key)}</Chip>:<span className="afya-stat-note">Not listed under a topic</span>}</td>
            <td>{c.sensitive?<Chip tone="warn">Sensitive health</Chip>:<Chip tone="neutral">General</Chip>}</td>
            <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
              <button type="button" className="afya-icon-btn" aria-label={`Edit ${c.name_en}`} disabled={busy} onClick={()=>requestNavigation(()=>{setSelected(c);setEn(c.name_en);setSw(c.name_sw);setSensitive(c.sensitive);setTopic(c.topic_key??'');})}><Pencil size={16}/></button>
              <button type="button" className="afya-icon-btn" aria-label={`Delete ${c.name_en}`} disabled={busy} onClick={()=>remove(c.id)}><Trash2 size={16}/></button>
            </td></tr>)}</tbody>
        </table>
        {!categories.length&&<EmptyState title="No categories yet">Create one to start publishing articles.</EmptyState>}
      </div>
    </Card>
    <Card title={selected?'Edit category':'New category'} subtitle="English and Kiswahili names are both required.">
      <fieldset disabled={busy} className="afya-form" style={{border:0,padding:0,margin:0}}>
        <label htmlFor="cat-en">English name</label><input id="cat-en" value={en} maxLength={80} onChange={e=>setEn(e.target.value)}/>
        <label htmlFor="cat-sw">Kiswahili name</label><input id="cat-sw" value={sw} maxLength={80} onChange={e=>setSw(e.target.value)}/>
        <label htmlFor="cat-topic">Patient app topic</label>
        <select id="cat-topic" value={topic} onChange={e=>setTopic(e.target.value as TopicKey|'')}><option value="">None (not listed under a topic)</option>{educationTopics.map(t=><option key={t.key} value={t.key}>{t.label}</option>)}</select>
        <label className="afya-check" style={{marginTop:12}}><input type="checkbox" disabled={Boolean(selected)} checked={sensitive} onChange={e=>setSensitive(e.target.checked)}/>Sensitive health category</label>
        <div style={{display:'flex',gap:10,marginTop:16}}>
          <button type="button" className="afya-btn-primary" disabled={busy||!en.trim()||!sw.trim()} onClick={save}>{selected?'Save category':'Create category'}</button>
          {selected&&<button type="button" className="afya-btn-secondary" onClick={reset}>Cancel</button>}
        </div>
      </fieldset>
    </Card>
  </div>;
}

/** Content → Topics: the six Patient app topics and the categories linked to each. */
export function TopicsPage(){
  const [categories,setCategories]=useState<ArticleCategory[]>([]);const [error,setError]=useState('');
  const reload=async()=>{setCategories(await listCategories());};
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  return <div className="afya-stack">
    <PageHead title="Topics & Categories" subtitle="Link article categories to the six HIV Prevention & Sexual Health topics in the patient app."/>
    {error&&<div className="afya-login-error" role="alert">{error}</div>}
    <Card title="Patient app topics" subtitle="A topic lists the published articles and videos of every category linked to it.">
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Topic</th><th>Linked categories</th></tr></thead>
          <tbody>{educationTopics.map(t=>{const linked=categories.filter(c=>c.topic_key===t.key);return <tr key={t.key}>
            <td style={{fontWeight:700}}>{t.label}</td>
            <td>{linked.length?<div style={{display:'flex',gap:6,flexWrap:'wrap'}}>{linked.map(c=><Chip key={c.id} tone="topic">{c.name_en}</Chip>)}</div>:<span className="afya-stat-note">No category linked: the topic shows its empty state</span>}</td>
          </tr>;})}</tbody>
        </table>
      </div>
    </Card>
    <CategoryManager categories={categories} reload={reload}/>
  </div>;
}
