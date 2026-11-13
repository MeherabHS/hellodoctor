'use client';
import {useEffect,useRef,useState} from 'react';
import type {AdminArticle,ArticleCategory} from '../../lib/blogContent';
import {articleStatuses,topicLabel} from '../../lib/blogContent';
import {listArticles,listCategories,getArticle} from '../../lib/blogApi';
import {ArticleEditor} from './ArticleEditor';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {blogErrorMessage} from '../../lib/blogErrors';
import './blogs.css';

/** Content → Quiz Manager: quizzes that stand on their own, not attached to a
 * blog article. Same question builder, review and publication rules as
 * article quizzes. Patients find them under HIV Prevention & Sexual Health →
 * Quizzes; the category decides the topic. */
export function QuizManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminArticle[]>([]);const [categories,setCategories]=useState<ArticleCategory[]>([]);
  const [status,setStatus]=useState('');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);const [total,setTotal]=useState(0);
  const [editor,setEditor]=useState<AdminArticle|null|undefined>(undefined);const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const query=new URLSearchParams({limit:'20',offset:String(offset),kind:'quiz',...(status?{status}:{}),...(search.trim()?{search:search.trim()}:{})});
    listArticles(query).then(r=>{if(active){setRows(r.items);setTotal(r.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },180);return()=>{active=false;clearTimeout(timer);};},[offset,status,search,revision]);
  async function open(id:string){const g=++generation.current;setLoading(true);try{const r=await getArticle(id);if(g===generation.current)setEditor(r);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const categoryOf=(a:AdminArticle)=>categories.find(c=>c.id===a.category_id);
  return <div className="cms-manager">
    {editor!==undefined?<ArticleEditor key={editor?.id||'new-quiz'} mode="quiz" initial={editor} categories={categories} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/>:<>
      <div className="cms-editor-heading"><div><h2>Quiz Manager</h2><p>Quizzes that stand on their own, not attached to a blog article. They appear in the Patient app under HIV Prevention &amp; Sexual Health → Quizzes.</p></div><button className="cms-primary" type="button" disabled={!categories.length} onClick={()=>requestNavigation(()=>setEditor(null))}>Create quiz</button></div>
      {!categories.length&&<p>Create a category in Blog Manager first; a category linked to a topic places the quiz under that topic.</p>}
      <div className="cms-filters"><label>Search titles<input type="search" value={search} maxLength={160} onChange={e=>{setSearch(e.target.value);setOffset(0);}}/></label><label>Status<select value={status} onChange={e=>{setStatus(e.target.value);setOffset(0);}}><option value="">All statuses</option>{articleStatuses.map(s=><option key={s}>{s}</option>)}</select></label></div>
      {error&&<p role="alert" className="cms-error">{error}<button type="button" onClick={()=>setRevision(v=>v+1)}>Retry</button></p>}
      {loading?<p role="status">Loading quizzes…</p>:<div className="cms-table-wrap"><table><thead><tr>{['Quiz','Topic','Status','Actions'].map(h=><th key={h}>{h}</th>)}</tr></thead><tbody>{rows.map(a=>{const c=categoryOf(a);return <tr key={a.id}><td><strong>{a.title_en}</strong><small>{c?.name_en||a.category_name_en as string}</small></td><td>{topicLabel(c?.topic_key)||'—'}</td><td><span className={`cms-status cms-status-${a.status.toLowerCase()}`}>{a.status.replaceAll('_',' ')}</span></td><td><button type="button" onClick={()=>requestNavigation(()=>open(a.id))}>Open</button></td></tr>;})}</tbody></table>{!rows.length&&<p>No quizzes yet.</p>}</div>}
      <div className="cms-pagination"><button type="button" disabled={offset===0||loading} onClick={()=>setOffset(v=>Math.max(0,v-20))}>Previous</button><span>{total?offset+1:0}–{Math.min(offset+20,total)} of {total}</span><button type="button" disabled={offset+20>=total||loading} onClick={()=>setOffset(v=>v+20)}>Next</button></div>
    </>}
  </div>;
}
