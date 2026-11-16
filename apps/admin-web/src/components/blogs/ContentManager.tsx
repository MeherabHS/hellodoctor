'use client';
import {useEffect,useRef,useState,type ComponentType} from 'react';
import {Archive,CalendarClock,CheckCircle2,FileEdit,Pencil,Plus,type LucideIcon} from 'lucide-react';
import type {AdminArticle,ArticleCategory,ArticleStatus} from '../../lib/blogContent';
import {topicLabel} from '../../lib/blogContent';
import {listArticles,listCategories,getArticle} from '../../lib/blogApi';
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

export type EditorProps={initial:AdminArticle|null;categories:ArticleCategory[];onSaved:()=>void;onClose:(message?:string)=>void};

/** Cover thumbnail through the queued Admin media route; an icon tile when there is no cover. */
export function CoverThumb({article,size=56,icon:Icon}:{article:Pick<AdminArticle,'id'|'cover_image_id'|'updated_at'>;size?:number;icon:LucideIcon}){
  const box={width:size,height:size,borderRadius:10,flexShrink:0} as const;
  const tile=<div aria-hidden style={{...box,display:'grid',placeItems:'center',background:'var(--afya-green-soft)',color:'var(--afya-green)'}}><Icon size={size*0.42}/></div>;
  if(!article.cover_image_id)return tile;
  return <QueuedImage src={`/api/article-media/${encodeURIComponent(article.id)}?v=${encodeURIComponent(article.cover_image_id)}`} alt="" style={{...box,objectFit:'cover'}} fallback={tile}/>;
}

/** List page for one kind of content (Articles, Videos or Quizzes) and the
 *  entry point to its editor. */
export function ContentManager({kind,title,subtitle,noun,icon,Editor,showTopic}:{kind:'article'|'video'|'quiz';title:string;subtitle:string;noun:string;icon:LucideIcon;Editor:ComponentType<EditorProps>;showTopic?:boolean}){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminArticle[]>([]);const [categories,setCategories]=useState<ArticleCategory[]>([]);
  const [status,setStatus]=useState<StatusTab>('');const [category,setCategory]=useState('');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);const [total,setTotal]=useState(0);
  const [counts,setCounts]=useState<Partial<Record<StatusTab,number>>>({});
  const [editor,setEditor]=useState<AdminArticle|null|undefined>(undefined);const [error,setError]=useState('');const [notice,setNotice]=useState('');
  const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[revision]);
  // Counts one status at a time to stay inside the per-user concurrency limit.
  useEffect(()=>{let active=true;
    (async()=>{for(const t of TABS){
      try{const r=await listArticles(new URLSearchParams({limit:'1',offset:'0',kind,...(t.id?{status:t.id}:{})}));if(!active)return;setCounts(prev=>({...prev,[t.id]:r.total}));}
      catch{if(!active)return;}
    }})();
    return()=>{active=false;};},[revision,kind]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const query=new URLSearchParams({limit:String(PAGE),offset:String(offset),kind,...(status?{status}:{}),...(category?{category_id:category}:{}),...(search.trim()?{search:search.trim()}:{})});
    listArticles(query).then(r=>{if(active){setRows(r.items);setTotal(r.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },180);return()=>{active=false;clearTimeout(timer);};},[offset,status,category,search,revision,kind]);
  async function open(id:string){const g=++generation.current;setLoading(true);try{const r=await getArticle(id);if(g===generation.current){setNotice('');setEditor(r);}}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const categoryOf=(a:AdminArticle)=>categories.find(c=>c.id===a.category_id);
  const topicCategories=categories.filter(c=>c.topic_key||rows.some(r=>r.category_id===c.id));

  if(editor!==undefined)return <Editor key={editor?.id||`new-${kind}`} initial={editor} categories={categories}
    onSaved={()=>setRevision(v=>v+1)} onClose={message=>{setEditor(undefined);setNotice(message??'');setRevision(v=>v+1);}}/>;

  const Icon=icon;const plural=noun==="quiz"?"quizzes":`${noun}s`;
  return <div className="afya-stack">
    <PageHead title={title} subtitle={subtitle}
      actions={<button type="button" className="afya-btn-primary" onClick={()=>requestNavigation(()=>{setNotice('');setEditor(null);})}><Plus size={16} aria-hidden/>New {noun}</button>}/>
    {notice&&<p role="status" className="afya-notice info" style={{margin:0}}>{notice}</p>}
    <div className="afya-stat-grid">
      <StatTile icon={Icon} tone="green" value={counts['']??'—'} label={`All ${plural}`}/>
      <StatTile icon={CheckCircle2} tone="green" value={counts.PUBLISHED??'—'} label="Published"/>
      <StatTile icon={FileEdit} tone="amber" value={counts.DRAFT??'—'} label="Drafts"/>
      <StatTile icon={CalendarClock} tone="blue" value={counts.SCHEDULED??'—'} label="Scheduled"/>
      <StatTile icon={Archive} tone="neutral" value={counts.ARCHIVED??'—'} label="Archived"/>
    </div>
    <section className="admin-card">
      <Tabs label={`${title} status`} value={status} onChange={v=>{setStatus(v);setOffset(0);}} items={TABS.map(t=>({...t,count:counts[t.id]}))}/>
      <div className="afya-filter-bar">
        <SearchField label={`Search ${plural}`} placeholder={`Search ${plural} by title…`} value={search} onChange={v=>{setSearch(v);setOffset(0);}}/>
        {showTopic&&<select className="afya-select" aria-label="Topic" value={category} onChange={e=>{setCategory(e.target.value);setOffset(0);}}>
          <option value="">All topics</option>
          {topicCategories.map(c=><option key={c.id} value={c.id}>{c.topic_key?topicLabel(c.topic_key):c.name_en}</option>)}
        </select>}
      </div>
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>{noun[0].toUpperCase()+noun.slice(1)}</th>{showTopic&&<th>Topic</th>}<th>Status</th><th>Updated</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{loading&&!rows.length?<tr><td colSpan={showTopic?5:4}><EmptyState title="Loading…"/></td></tr>:rows.map(a=>{
            const c=categoryOf(a);const chip=STATUS_CHIP[a.status];
            return <tr key={a.id} className="admin-clickable-row" onClick={()=>requestNavigation(()=>open(a.id))}>
              <td><div style={{display:'flex',alignItems:'center',gap:12}}><CoverThumb article={a} icon={Icon}/><div style={{minWidth:0}}>
                <div style={{fontWeight:700,color:'var(--afya-ink)'}}>{a.title_en}</div>
                <div className="afya-stat-note" style={{maxWidth:420,overflow:'hidden',textOverflow:'ellipsis',whiteSpace:'nowrap'}}>{kind==='article'?(a.title_sw?'English · Kiswahili':'English'):a.body_en}</div></div></div></td>
              {showTopic&&<td>{c?<Chip tone="topic">{c.topic_key?topicLabel(c.topic_key):c.name_en}</Chip>:'—'}</td>}
              <td><Chip tone={chip.tone}>{chip.label}</Chip></td>
              <td style={{whiteSpace:'nowrap'}}>{eatDate(a.updated_at)}</td>
              <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={`Edit ${a.title_en}`} onClick={e=>{e.stopPropagation();requestNavigation(()=>open(a.id));}}><Pencil size={16}/></button></td>
            </tr>;})}</tbody>
        </table>
        {!loading&&!rows.length&&<EmptyState title={`No ${plural} here yet`}>Press New {noun} to add one.</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={total} busy={loading} onChange={setOffset}/>
    </section>
  </div>;
}
