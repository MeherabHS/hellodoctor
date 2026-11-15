'use client';
import {useEffect,useRef,useState} from 'react';
import {Archive,BookOpen,CalendarClock,CheckCircle2,FileEdit,Pencil,Plus} from 'lucide-react';
import type {AdminArticle,ArticleCategory,ArticleStatus} from '../../lib/blogContent';
import {topicLabel} from '../../lib/blogContent';
import {listArticles,listCategories,getArticle} from '../../lib/blogApi';
import {ArticleEditor} from './ArticleEditor';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import {Chip,EmptyState,PageHead,Pagination,QueuedImage,SearchField,StatTile,Tabs,eatDate} from '../ui/afya';
import {STATUS_CHIP} from './statusChip';
import './blogs.css';

const PAGE=20;
type StatusTab=''|ArticleStatus;
const TABS:{id:StatusTab;label:string}[]=[
  {id:'',label:'All'},{id:'DRAFT',label:'Draft'},
  {id:'SCHEDULED',label:'Scheduled'},{id:'PUBLISHED',label:'Published'},{id:'ARCHIVED',label:'Archived'},
];

/** Cover thumbnail through the queued Admin media route; a neutral tile when there is no cover. */
export function CoverThumb({article,size=56}:{article:Pick<AdminArticle,'id'|'cover_image_id'|'updated_at'>;size?:number}){
  const box={width:size,height:size,borderRadius:10,flexShrink:0} as const;
  const tile=<div aria-hidden style={{...box,display:'grid',placeItems:'center',background:'var(--afya-green-soft)',color:'var(--afya-green)'}}><BookOpen size={size*0.42}/></div>;
  if(!article.cover_image_id)return tile;
  return <QueuedImage src={`/api/article-media/${encodeURIComponent(article.id)}?v=${encodeURIComponent(article.cover_image_id)}`} alt="" style={{...box,objectFit:'cover'}} fallback={tile}/>;
}

export function BlogManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminArticle[]>([]);const [categories,setCategories]=useState<ArticleCategory[]>([]);
  const [status,setStatus]=useState<StatusTab>('');const [category,setCategory]=useState('');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);const [total,setTotal]=useState(0);
  const [counts,setCounts]=useState<Partial<Record<StatusTab,number>>>({});
  const [editor,setEditor]=useState<AdminArticle|null|undefined>(undefined);const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[revision]);
  // Tab and tile counts: one total-only request per status (limit 1), sent one
  // after another so the burst stays inside the backend's per-user concurrency
  // limit. A failed count stays "—"; it never hides the list.
  useEffect(()=>{let active=true;
    (async()=>{for(const t of TABS){
      try{const r=await listArticles(new URLSearchParams({limit:'1',offset:'0',kind:'content',...(t.id?{status:t.id}:{})}));if(!active)return;setCounts(prev=>({...prev,[t.id]:r.total}));}
      catch{if(!active)return;}
    }})();
    return()=>{active=false;};},[revision]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const query=new URLSearchParams({limit:String(PAGE),offset:String(offset),kind:'content',...(status?{status}:{}),...(category?{category_id:category}:{}),...(search.trim()?{search:search.trim()}: {})});
    listArticles(query).then(result=>{if(active){setRows(result.items);setTotal(result.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },180);return()=>{active=false;clearTimeout(timer);};},[offset,status,category,search,revision]);
  async function open(id:string){const current=++generation.current;setLoading(true);try{const result=await getArticle(id);if(current===generation.current)setEditor(result);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const categoryOf=(a:AdminArticle)=>categories.find(c=>c.id===a.category_id);

  if(editor!==undefined)return <div><ArticleEditor key={editor?.id||'new'} initial={editor} categories={categories} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/></div>;

  return <div className="afya-stack">
    <PageHead title="Articles (Blog)" subtitle="Create, edit, publish and manage health articles for the patient app."
      actions={<button type="button" className="afya-btn-primary" disabled={!categories.length} onClick={()=>requestNavigation(()=>setEditor(null))}><Plus size={16} aria-hidden/>New Article</button>}/>

    <div className="afya-stat-grid">
      <StatTile icon={BookOpen} tone="green" value={counts['']??'—'} label="Total articles"/>
      <StatTile icon={CheckCircle2} tone="green" value={counts.PUBLISHED??'—'} label="Published"/>
      <StatTile icon={FileEdit} tone="amber" value={counts.DRAFT??'—'} label="Drafts"/>
      <StatTile icon={CalendarClock} tone="blue" value={counts.SCHEDULED??'—'} label="Scheduled"/>
      <StatTile icon={Archive} tone="neutral" value={counts.ARCHIVED??'—'} label="Archived"/>
    </div>

    <section className="admin-card">
      <Tabs label="Article status" value={status} onChange={v=>{setStatus(v);setOffset(0);}} items={TABS.map(t=>({...t,count:counts[t.id]}))}/>
      <div className="afya-filter-bar">
        <SearchField label="Search articles" placeholder="Search articles by title or keyword…" value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
        <select className="afya-select" aria-label="Topic or category" value={category} onChange={e=>{setCategory(e.target.value);setOffset(0);}}>
          <option value="">All topics</option>
          {categories.map(c=><option key={c.id} value={c.id}>{c.topic_key?`${topicLabel(c.topic_key)} · ${c.name_en}`:c.name_en}</option>)}
        </select>
      </div>
      {!categories.length&&!error&&<p className="afya-stat-note" style={{margin:'0 0 12px'}}>Create a category in Content → Topics before your first article.</p>}
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Article</th><th>Topic</th><th>Language</th><th>Status</th><th>Updated</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>
            {loading&&!rows.length?<tr><td colSpan={6}><EmptyState title="Loading articles…"/></td></tr>:rows.map(a=>{
              const c=categoryOf(a);const chip=STATUS_CHIP[a.status];
              return <tr key={a.id} className="admin-clickable-row" onClick={()=>requestNavigation(()=>open(a.id))}>
                <td><div style={{display:'flex',alignItems:'center',gap:12}}><CoverThumb article={a}/><div style={{minWidth:0}}>
                  <div style={{fontWeight:700,color:'var(--afya-ink)'}}>{a.title_en}</div>
                  <div className="afya-stat-note">{a.video_url?'Video · ':''}{a.author_name}{a.featured?' · Featured':''}</div></div></div></td>
                <td>{c?<Chip tone="topic">{c.topic_key?topicLabel(c.topic_key):c.name_en}</Chip>:'—'}</td>
                <td><div style={{display:'flex',gap:6}}><Chip tone="outlined">EN</Chip>{a.title_sw&&a.body_sw&&<Chip tone="outlined">SW</Chip>}</div></td>
                <td><Chip tone={chip.tone}>{chip.label}</Chip></td>
                <td style={{whiteSpace:'nowrap'}}>{eatDate(a.updated_at)}</td>
                <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={`Edit ${a.title_en}`} onClick={e=>{e.stopPropagation();requestNavigation(()=>open(a.id));}}><Pencil size={16}/></button></td>
              </tr>;})}
          </tbody>
        </table>
        {!loading&&!rows.length&&<EmptyState title="No articles match these filters">Try another status, topic or search.</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={total} busy={loading} onChange={setOffset}/>
      <p className="afya-stat-note" style={{marginTop:12}}>Publishing sends no push notification; campaigns are a separate action.</p>
    </section>
  </div>;
}
