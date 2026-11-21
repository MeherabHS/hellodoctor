'use client';
import { tr } from '@/lib/i18n';
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
  async function remove(id:string){if(!window.confirm(tr('ui.deleteThisUnusedCategory')))return;setBusy(true);try{await deleteCategory(id);await reload();}catch(e){setError(blogErrorMessage(e));}finally{setBusy(false);}}
  return <div className="afya-two-col">
    <Card title={tr('ui.categories')} subtitle={tr('ui4.articlesAndVideosInACategory')}>
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error}</div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.category')}</th><th>{tr('ui.patientAppTopic')}</th><th>{tr('ui.label')}</th><th style={{textAlign:'right'}}>{tr('ui.actions')}</th></tr></thead>
          <tbody>{categories.map(c=><tr key={c.id}>
            <td><div style={{fontWeight:700}}>{c.name_en}</div><div className="afya-stat-note">{c.name_sw}</div></td>
            <td>{c.topic_key?<Chip tone="topic">{topicLabel(c.topic_key)}</Chip>:<span className="afya-stat-note">{tr('ui.notListedUnderATopic')}</span>}</td>
            <td>{c.sensitive?<Chip tone="warn">{tr('ui.sensitiveHealth')}</Chip>:<Chip tone="neutral">{tr('ui.general')}</Chip>}</td>
            <td style={{textAlign:'right',whiteSpace:'nowrap'}}>
              <button type="button" className="afya-icon-btn" aria-label={tr('tpl.editNamed', { name: c.name_en })} disabled={busy} onClick={()=>requestNavigation(()=>{setSelected(c);setEn(c.name_en);setSw(c.name_sw);setSensitive(c.sensitive);setTopic(c.topic_key??'');})}><Pencil size={16}/></button>
              <button type="button" className="afya-icon-btn" aria-label={tr('tpl.deleteNamed', { name: c.name_en })} disabled={busy} onClick={()=>remove(c.id)}><Trash2 size={16}/></button>
            </td></tr>)}</tbody>
        </table>
        {!categories.length&&<EmptyState title={tr('ui.noCategoriesYet')}>{tr('ui.createOneToStartPublishingArticles')}</EmptyState>}
      </div>
    </Card>
    <Card title={selected?tr('ui2.editCategory'):tr('ui2.newCategory')} subtitle={tr('ui4.englishAndKiswahiliNamesAreBoth')}>
      <fieldset disabled={busy} className="afya-form" style={{border:0,padding:0,margin:0}}>
        <label htmlFor="cat-en">{tr('ui.englishName')}</label><input id="cat-en" value={en} maxLength={80} onChange={e=>setEn(e.target.value)}/>
        <label htmlFor="cat-sw">{tr('ui.kiswahiliName')}</label><input id="cat-sw" value={sw} maxLength={80} onChange={e=>setSw(e.target.value)}/>
        <label htmlFor="cat-topic">{tr('ui.patientAppTopic')}</label>
        <select id="cat-topic" value={topic} onChange={e=>setTopic(e.target.value as TopicKey|'')}><option value="">{tr('ui.noneNotListedUnderATopic')}</option>{educationTopics.map(t=><option key={t.key} value={t.key}>{topicLabel(t.key)}</option>)}</select>
        <label className="afya-check" style={{marginTop:12}}><input type="checkbox" disabled={Boolean(selected)} checked={sensitive} onChange={e=>setSensitive(e.target.checked)}/>{tr('ui.sensitiveHealthCategory')}</label>
        <div style={{display:'flex',gap:10,marginTop:16}}>
          <button type="button" className="afya-btn-primary" disabled={busy||!en.trim()||!sw.trim()} onClick={save}>{selected?tr('ui2.saveCategory'):tr('ui2.createCategory')}</button>
          {selected&&<button type="button" className="afya-btn-secondary" onClick={reset}>{tr('ui.cancel')}</button>}
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
    <PageHead title={tr('ui.topicsCategories')} subtitle={tr('ui4.linkArticleCategoriesToTheSix')}/>
    {error&&<div className="afya-login-error" role="alert">{error}</div>}
    <Card title={tr('ui.patientAppTopics')} subtitle={tr('ui4.aTopicListsThePublishedArticles')}>
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{tr('ui.topic2')}</th><th>{tr('ui.linkedCategories')}</th></tr></thead>
          <tbody>{educationTopics.map(t=>{const linked=categories.filter(c=>c.topic_key===t.key);return <tr key={t.key}>
            <td style={{fontWeight:700}}>{topicLabel(t.key)}</td>
            <td>{linked.length?<div style={{display:'flex',gap:6,flexWrap:'wrap'}}>{linked.map(c=><Chip key={c.id} tone="topic">{c.name_en}</Chip>)}</div>:<span className="afya-stat-note">{tr('ui.noCategoryLinkedTheTopicShows')}</span>}</td>
          </tr>;})}</tbody>
        </table>
      </div>
    </Card>
    <CategoryManager categories={categories} reload={reload}/>
  </div>;
}
