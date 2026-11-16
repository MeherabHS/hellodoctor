'use client';
import {useState} from 'react';
import type {AdminArticle} from '../../lib/blogContent';
import {isUnpublished} from '../../lib/publishFlow';
import {Card,Chip} from '../ui/afya';
import {STATUS_CHIP} from './statusChip';

/** Status and actions for an article, video or quiz: Save draft, Publish
 *  (saves first), and for published entries Unpublish / Archive. */
export function PublishCard({article,noun,busy,canSave,problems,onSave,onPublish,onSchedule,onAction}:{
  article:AdminArticle|null;noun:string;busy:boolean;canSave:boolean;problems:string[];
  onSave:()=>void;onPublish:()=>void;onSchedule?:(when:string)=>void;onAction:(action:'unpublish'|'archive')=>void;
}){
  const [later,setLater]=useState('');
  const status=article?STATUS_CHIP[article.status]:STATUS_CHIP.DRAFT;
  const archived=article?.status==='ARCHIVED';const live=article&&!isUnpublished(article.status)&&!archived;
  return <Card title="Publish">
    <div className="afya-stack" style={{gap:12}}>
      <div style={{display:'flex',alignItems:'center',gap:8,flexWrap:'wrap'}}>
        <Chip tone={status.tone}>{status.label}</Chip>
        {article?.scheduled_for&&<span className="afya-stat-note">{new Date(article.scheduled_for).toLocaleString('en-KE',{timeZone:'Africa/Nairobi'})} EAT</span>}
      </div>
      {problems.length>0&&<ul className="cms-problems">{problems.map(p=><li key={p}>{p}</li>)}</ul>}
      {!archived&&<div style={{display:'flex',gap:10}}>
        {/* Saving a live entry would take it off the Patient app; Update saves and republishes. */}
        {!live&&<button type="button" className="afya-btn-outline-green" style={{flex:1}} disabled={busy||!canSave} onClick={onSave}>Save draft</button>}
        <button type="button" className="afya-btn-primary" style={{flex:1}} disabled={busy||problems.length>0||(Boolean(live)&&!canSave)} onClick={onPublish}>{busy?'Working…':live?'Update':'Publish'}</button>
      </div>}
      {live&&<span className="afya-stat-note">Live in the Patient app. Changes go live when you press Update.</span>}
      {!live&&!archived&&onSchedule&&<details className="cms-later">
        <summary>Publish later</summary>
        <label htmlFor="publish-later" className="afya-field-label">Date and time · Africa/Nairobi</label>
        <div style={{display:'flex',gap:8}}>
          <input id="publish-later" className="afya-input" type="datetime-local" value={later} onChange={e=>setLater(e.target.value)} style={{flex:1}}/>
          <button type="button" className="afya-btn-secondary" disabled={busy||!later||problems.length>0} onClick={()=>onSchedule(later)}>Schedule</button>
        </div>
      </details>}
      {article&&!archived&&<div style={{display:'flex',gap:8,flexWrap:'wrap',borderTop:'1px solid var(--afya-line-soft)',paddingTop:12}}>
        {live&&<button type="button" className="afya-btn-secondary" disabled={busy} onClick={()=>onAction('unpublish')}>Unpublish</button>}
        <button type="button" className="afya-btn-danger" disabled={busy} onClick={()=>{if(window.confirm(`Archive this ${noun}? Archived items cannot be edited.`))onAction('archive');}}>Archive</button>
      </div>}
      {archived&&<span className="afya-stat-note">Archived. It is no longer shown to patients and cannot be edited.</span>}
      <span className="afya-stat-note">Publishing sends no notification to patients.</span>
    </div>
  </Card>;
}
