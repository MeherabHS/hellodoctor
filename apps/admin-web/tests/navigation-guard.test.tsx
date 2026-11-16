import React,{useState} from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {NavigationGuard,useAdminNavigation} from '../src/components/navigation/NavigationGuard';
import {ArticleEditor} from '../src/components/blogs/ArticleEditor';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';

const category={id:'category-a',name_en:'Education',name_sw:'Elimu',sensitive:false,topic_key:null};
const prep={id:'category-prep',name_en:'PrEP',name_sw:'PrEP',sensitive:true,topic_key:'PREP' as const};
const article={...blankArticle(),id:'00000000-0000-4000-8000-000000000001',title_en:'Saved title',body_en:'Saved body',category_id:category.id,status:'DRAFT',version:1,content_version:1} as AdminArticle;
// Buttons match their text, or their aria-label for icon-only buttons.
function button(v:ReactTestRenderer,label:string){return v.root.findAllByType('button').find(n=>n.children.join('')===label||n.props['aria-label']===label)!;}
function Harness({initial=article}:{initial?:AdminArticle|null}) {
  const [open,setOpen]=useState(true);const {requestNavigation}=useAdminNavigation();
  return <><button onClick={()=>requestNavigation(()=>setOpen(false))}>Sidebar destination</button>{open?<ArticleEditor initial={initial} categories={[category,prep]} onSaved={()=>{}} onClose={()=>setOpen(false)}/>:<p>Destination</p>}</>;
}
const mount=async(initial:AdminArticle|null=article)=>{let v!:ReactTestRenderer;await act(async()=>{v=create(<NavigationGuard><Harness initial={initial}/></NavigationGuard>);});return v;};
const edit=(v:ReactTestRenderer,id:string,value:unknown)=>v.root.find(n=>n.props.id===id&&['input','textarea','select'].includes(String(n.type))).props.onChange({target:{value}});
const dialogs=(v:ReactTestRenderer)=>v.root.findAllByProps({role:'alertdialog'}).length;
const editors=(v:ReactTestRenderer)=>v.root.findAllByType(ArticleEditor).length;

for(const [label,id,value] of [['title','art-title','Unsaved title'],['article text','art-body','Unsaved body'],['topic','art-topic','PREP']] as const){
  test(`navigation protects an unsaved ${label}; Keep editing retains it, Discard leaves`,async()=>{
    const v=await mount();try{
      await act(async()=>edit(v,id,value));await act(async()=>button(v,'Sidebar destination').props.onClick());
      assert.equal(dialogs(v),1);
      await act(async()=>button(v,'Keep editing').props.onClick());assert.equal(editors(v),1);
      await act(async()=>button(v,'Sidebar destination').props.onClick());await act(async()=>button(v,'Discard changes').props.onClick());assert.equal(editors(v),0);
    }finally{act(()=>v.unmount());}
  });
}
test('untouched new article and reverted edits leave without a warning',async()=>{
  for(const initial of [null,article]){const v=await mount(initial);try{if(initial){await act(async()=>edit(v,'art-title','changed'));await act(async()=>edit(v,'art-title','Saved title'));}await act(async()=>button(v,'Back to articles').props.onClick());assert.equal(dialogs(v),0);assert.equal(editors(v),0);}finally{act(()=>v.unmount());}}
});
test('a pending save prevents discard; failure keeps the unsaved draft',async()=>{
  const old=globalThis.fetch;let reject!:(e:Error)=>void;globalThis.fetch=async()=>new Promise<Response>((_r,j)=>{reject=j;});const v=await mount();
  try{
    await act(async()=>edit(v,'art-title','Keep this draft'));
    let saving:Promise<void>|undefined;act(()=>{saving=button(v,'Save draft').props.onClick();});
    await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(button(v,'Discard changes').props.disabled,true);
    await act(async()=>button(v,'Discard changes').props.onClick());assert.equal(editors(v),1);
    await act(async()=>{reject(new Error('Synthetic save failure'));await saving;});assert.equal(button(v,'Discard changes').props.disabled,false);
    await act(async()=>button(v,'Keep editing').props.onClick());assert.ok(JSON.stringify(v.toJSON()).includes('Keep this draft'));
    await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(dialogs(v),1);
  } finally {act(()=>v.unmount());globalThis.fetch=old;}
});
// Regression (2026-11-16): after publishing, going back asked "Leave this editor?".
test('publishing returns to the list with no leave prompt',async()=>{
  const old=globalThis.fetch;let version=1;
  globalThis.fetch=async(url)=>{version++;const path=String(url);const status=path.endsWith('/submit')?'PENDING_REVIEW':path.endsWith('/approve')?'APPROVED':path.endsWith('/publish')?'PUBLISHED':'DRAFT';
    return new Response(JSON.stringify({success:true,data:{...article,title_en:'Edited',status,version}}));};
  const v=await mount();
  try{
    await act(async()=>edit(v,'art-title','Edited'));
    await act(async()=>button(v,'Publish').props.onClick());
    assert.equal(editors(v),0,'back on the list');assert.equal(dialogs(v),0,'no leave prompt');
    await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(dialogs(v),0);
  }finally{act(()=>v.unmount());globalThis.fetch=old;}
});
