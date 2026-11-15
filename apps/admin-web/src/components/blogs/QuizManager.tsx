'use client';
import {useEffect,useRef,useState} from 'react';
import {CheckCircle2,ClipboardList,FileEdit,Hourglass,ListChecks,Pencil,Plus} from 'lucide-react';
import type {AdminArticle,ArticleCategory,ArticleStatus} from '../../lib/blogContent';
import {topicLabel} from '../../lib/blogContent';
import {listArticles,listCategories,getArticle} from '../../lib/blogApi';
import {ArticleEditor} from './ArticleEditor';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import {Chip,EmptyState,PageHead,Pagination,SearchField,StatTile,Tabs,eatDate} from '../ui/afya';
import {STATUS_CHIP} from './statusChip';
import './blogs.css';

const PAGE=20;
type StatusTab=''|ArticleStatus;
const TABS:{id:StatusTab;label:string}[]=[
  {id:'',label:'All'},{id:'DRAFT',label:'Draft'},
  {id:'SCHEDULED',label:'Scheduled'},{id:'PUBLISHED',label:'Published'},{id:'ARCHIVED',label:'Archived'},
];

/** Content → Quizzes: quizzes that stand on their own, not attached to a
 * blog article. Same question builder, review and publication rules as
 * article quizzes. Patients find them under HIV Prevention & Sexual Health →
 * Quizzes; the category decides the topic. */
export function QuizManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminArticle[]>([]);const [categories,setCategories]=useState<ArticleCategory[]>([]);
  const [status,setStatus]=useState<StatusTab>('');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);const [total,setTotal]=useState(0);
  const [counts,setCounts]=useState<Partial<Record<StatusTab,number>>>({});
  const [editor,setEditor]=useState<AdminArticle|null|undefined>(undefined);const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  // Counts one status at a time to stay inside the per-user concurrency limit.
  useEffect(()=>{let active=true;
    (async()=>{for(const t of TABS){
      try{const r=await listArticles(new URLSearchParams({limit:'1',offset:'0',kind:'quiz',...(t.id?{status:t.id}:{})}));if(!active)return;setCounts(prev=>({...prev,[t.id]:r.total}));}
      catch{if(!active)return;}
    }})();
    return()=>{active=false;};},[revision]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const query=new URLSearchParams({limit:String(PAGE),offset:String(offset),kind:'quiz',...(status?{status}:{}),...(search.trim()?{search:search.trim()}:{})});
    listArticles(query).then(r=>{if(active){setRows(r.items);setTotal(r.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },180);return()=>{active=false;clearTimeout(timer);};},[offset,status,search,revision]);
  async function open(id:string){const g=++generation.current;setLoading(true);try{const r=await getArticle(id);if(g===generation.current)setEditor(r);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const categoryOf=(a:AdminArticle)=>categories.find(c=>c.id===a.category_id);

  if(editor!==undefined)return <div><ArticleEditor key={editor?.id||'new-quiz'} mode="quiz" initial={editor} categories={categories} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/></div>;

  return <div className="afya-stack">
    <PageHead title="Quizzes" subtitle="Standalone quizzes for the patient app, listed under HIV Prevention & Sexual Health → Quizzes."
      actions={<button type="button" className="afya-btn-primary" disabled={!categories.length} onClick={()=>requestNavigation(()=>setEditor(null))}><Plus size={16} aria-hidden/>New Quiz</button>}/>
    <div className="afya-stat-grid">
      <StatTile icon={ClipboardList} tone="green" value={counts['']??'—'} label="Total quizzes"/>
      <StatTile icon={CheckCircle2} tone="green" value={counts.PUBLISHED??'—'} label="Published"/>
      <StatTile icon={FileEdit} tone="amber" value={counts.DRAFT??'—'} label="Drafts"/>
    </div>
    <section className="admin-card">
      <Tabs label="Quiz status" value={status} onChange={v=>{setStatus(v);setOffset(0);}} items={TABS.map(t=>({...t,count:counts[t.id]}))}/>
      <div className="afya-filter-bar"><SearchField label="Search quizzes" placeholder="Search quizzes by title…" value={search} onChange={v=>{setSearch(v);setOffset(0);}}/></div>
      {!categories.length&&!error&&<p className="afya-stat-note" style={{margin:'0 0 12px'}}>Create a category in Content → Topics first; a category linked to a topic places the quiz under that topic.</p>}
      {error&&<div className="afya-login-error" role="alert" style={{marginBottom:12}}>{error} <button type="button" className="afya-btn-secondary" onClick={()=>setRevision(v=>v+1)}>Retry</button></div>}
      <div className="afya-table-wrap">
        <table className="admin-table">
          <thead><tr><th>Quiz</th><th>Topic</th><th>Status</th><th>Updated</th><th style={{textAlign:'right'}}>Actions</th></tr></thead>
          <tbody>{loading&&!rows.length?<tr><td colSpan={5}><EmptyState title="Loading quizzes…"/></td></tr>:rows.map(a=>{const c=categoryOf(a);const chip=STATUS_CHIP[a.status];return <tr key={a.id} className="admin-clickable-row" onClick={()=>requestNavigation(()=>open(a.id))}>
            <td><div style={{display:'flex',alignItems:'center',gap:12}}><div aria-hidden style={{width:44,height:44,borderRadius:10,display:'grid',placeItems:'center',flexShrink:0}} className="afya-tone-purple"><ListChecks size={20}/></div>
              <div><div style={{fontWeight:700}}>{a.title_en}</div><div className="afya-stat-note">{c?.name_en||a.category_name_en as string}</div></div></div></td>
            <td>{c?.topic_key?<Chip tone="topic">{topicLabel(c.topic_key)}</Chip>:<span className="afya-stat-note">No topic</span>}</td>
            <td><Chip tone={chip.tone}>{chip.label}</Chip></td>
            <td style={{whiteSpace:'nowrap'}}>{eatDate(a.updated_at)}</td>
            <td style={{textAlign:'right'}}><button type="button" className="afya-icon-btn" aria-label={`Edit ${a.title_en}`} onClick={e=>{e.stopPropagation();requestNavigation(()=>open(a.id));}}><Pencil size={16}/></button></td>
          </tr>;})}</tbody>
        </table>
        {!loading&&!rows.length&&<EmptyState title="No quizzes yet">Create a quiz to add it to the patient app.</EmptyState>}
      </div>
      <Pagination offset={offset} pageSize={PAGE} total={total} busy={loading} onChange={setOffset}/>
    </section>
  </div>;
}
