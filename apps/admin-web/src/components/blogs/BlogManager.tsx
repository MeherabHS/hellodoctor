'use client';
import {useCallback,useEffect,useRef,useState} from 'react';
import type {AdminArticle,ArticleCategory} from '../../lib/blogContent';
import {articleStatuses} from '../../lib/blogContent';
import {listArticles,listCategories,getArticle} from '../../lib/blogApi';
import {ArticleEditor} from './ArticleEditor';
import {useAdminNavigation} from '../navigation/NavigationGuard';
import {CategoryManager} from './CategoryManager';
import './blogs.css';
import {blogErrorMessage} from '../../lib/blogErrors';

export function BlogManager(){
  const {requestNavigation}=useAdminNavigation();
  const [rows,setRows]=useState<AdminArticle[]>([]);const [categories,setCategories]=useState<ArticleCategory[]>([]);
  const [status,setStatus]=useState('');const [category,setCategory]=useState('');const [search,setSearch]=useState('');const [offset,setOffset]=useState(0);const [total,setTotal]=useState(0);
  const [editor,setEditor]=useState<AdminArticle|null|undefined>(undefined);const [error,setError]=useState('');const [loading,setLoading]=useState(true);const [revision,setRevision]=useState(0);const generation=useRef(0);
  const reloadCategories=useCallback(async()=>{setCategories(await listCategories());},[]);
  useEffect(()=>{let active=true;listCategories().then(c=>{if(active)setCategories(c);}).catch(e=>{if(active)setError(blogErrorMessage(e));});return()=>{active=false;};},[]);
  useEffect(()=>{let active=true;const timer=setTimeout(()=>{
    setLoading(true);setError('');
    const query=new URLSearchParams({limit:'20',offset:String(offset),...(status?{status}:{}),...(category?{category_id:category}:{}),...(search.trim()?{search:search.trim()}: {})});
    listArticles(query).then(result=>{if(active){setRows(result.items);setTotal(result.total);}}).catch(e=>{if(active)setError(blogErrorMessage(e));}).finally(()=>{if(active)setLoading(false);});
  },180);return()=>{active=false;clearTimeout(timer);};},[offset,status,category,search,revision]);
  async function open(id:string){const current=++generation.current;setLoading(true);try{const result=await getArticle(id);if(current===generation.current)setEditor(result);}catch(e){setError(blogErrorMessage(e));}finally{setLoading(false);}}
  const date=(value:string|null)=>value?new Date(value).toLocaleDateString('en-KE',{timeZone:'Africa/Nairobi'}):'—';
  return <div className="cms-manager">
    {editor!==undefined?<ArticleEditor key={editor?.id||'new'} initial={editor} categories={categories} onSaved={()=>setRevision(v=>v+1)} onClose={()=>setEditor(undefined)}/>:<>
      <div className="cms-editor-heading"><div><h2>Blog Manager</h2><p>Author, review and publish health articles in English and Kiswahili.</p></div><button className="cms-primary" type="button" disabled={!categories.length} onClick={()=>requestNavigation(()=>setEditor(null))}>Create article</button></div>
      <CategoryManager categories={categories} reload={reloadCategories}/>
      {!categories.length&&<p>Create a category before your first article.</p>}
      <div className="cms-filters"><label>Search titles<input type="search" value={search} maxLength={160} onChange={e=>{setSearch(e.target.value);setOffset(0);}}/></label><label>Publication status<select value={status} onChange={e=>{setStatus(e.target.value);setOffset(0);}}><option value="">All statuses</option>{articleStatuses.map(s=><option key={s}>{s}</option>)}</select></label><label>Category<select value={category} onChange={e=>{setCategory(e.target.value);setOffset(0);}}><option value="">All categories</option>{categories.map(c=><option key={c.id} value={c.id}>{c.name_en}</option>)}</select></label></div>
      {error&&<p role="alert" className="cms-error">{error}<button type="button" onClick={()=>setRevision(v=>v+1)}>Retry</button></p>}
      {loading?<p role="status">Loading articles…</p>:<div className="cms-table-wrap"><table><thead><tr>{['Article','Languages','Author','Status','Created','Published','Featured','Actions'].map(h=><th key={h}>{h}</th>)}</tr></thead><tbody>{rows.map(a=><tr key={a.id}><td><strong>{a.title_en}</strong><small>{a.category_name_en as string||categories.find(c=>c.id===a.category_id)?.name_en}</small></td><td>{a.title_sw&&a.body_sw?'EN · SW':'EN'}</td><td>{a.author_name}<small>{a.author_id}</small></td><td><span className={`cms-status cms-status-${a.status.toLowerCase()}`}>{a.status.replaceAll('_',' ')}</span>{a.status==='PENDING_REVIEW'&&<small>Awaiting approval</small>}</td><td>{date(a.created_at)}</td><td>{date(a.published_at)}</td><td>{a.featured?'Yes':'—'}</td><td><button type="button" onClick={()=>requestNavigation(()=>open(a.id))}>Open / preview</button></td></tr>)}</tbody></table>{!rows.length&&<p>No articles match these filters.</p>}</div>}
      <div className="cms-pagination"><button type="button" disabled={offset===0||loading} onClick={()=>setOffset(v=>Math.max(0,v-20))}>Previous</button><span>{total?offset+1:0}–{Math.min(offset+20,total)} of {total}</span><button type="button" disabled={offset+20>=total||loading} onClick={()=>setOffset(v=>v+20)}>Next</button></div>
      <p className="cms-hint">Publication and notification campaigns are separate actions. Publishing sends no push notification.</p>
    </>}
  </div>;
}
