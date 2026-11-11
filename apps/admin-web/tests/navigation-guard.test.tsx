import React,{useState} from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {NavigationGuard,useAdminNavigation} from '../src/components/navigation/NavigationGuard';
import {ArticleEditor} from '../src/components/blogs/ArticleEditor';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';

const category={id:'category-a',name_en:'Education',name_sw:'Elimu',sensitive:false};
const article={...blankArticle(),id:'00000000-0000-4000-8000-000000000001',title_en:'Saved title',body_en:'Saved body',category_id:category.id,status:'DRAFT',version:1,content_version:1} as AdminArticle;
function button(v:ReactTestRenderer,label:string){return v.root.findAllByType('button').find(n=>n.children.join('')===label)!;}
function Harness({initial=article}:{initial?:AdminArticle|null}) {
  const [open,setOpen]=useState(true);const {requestNavigation}=useAdminNavigation();
  return <><button onClick={()=>requestNavigation(()=>setOpen(false))}>Sidebar destination</button>{open?<ArticleEditor initial={initial} categories={[category,{...category,id:'category-b'}]} onSaved={()=>{}} onClose={()=>setOpen(false)}/>:<p>Destination</p>}</>;
}
const mount=async(initial:AdminArticle|null=article)=>{let v!:ReactTestRenderer;await act(async()=>{v=create(<NavigationGuard><Harness initial={initial}/></NavigationGuard>);});return v;};
const edit=(v:ReactTestRenderer,label:string,type:'input'|'textarea'|'select',value:unknown)=>{
  const node=v.root.findAllByType('label').find(n=>n.children.filter(c=>typeof c==='string').join('').startsWith(label))!;
  node.findByType(type).props.onChange({target:{value}});
};

for(const [label,type,value] of [['English title','input','Unsaved title'],['English content','textarea','Unsaved body'],['Tags','input','unsaved'],['Category','select','category-b']] as const){
  test(`navigation protects ${label}, Stay retains it, explicit discard unmounts`,async()=>{
    const v=await mount();try{
      await act(async()=>edit(v,label,type,value));await act(async()=>button(v,'Sidebar destination').props.onClick());
      assert.equal(v.root.findAllByProps({role:'alertdialog'}).length,1);
      await act(async()=>button(v,'Stay').props.onClick());assert.equal(v.root.findAllByType(ArticleEditor).length,1);
      await act(async()=>button(v,'Sidebar destination').props.onClick());await act(async()=>button(v,'Discard and Leave').props.onClick());assert.equal(v.root.findAllByType(ArticleEditor).length,0);
    }finally{act(()=>v.unmount());}
  });
}
test('untouched new article and reverted article edits leave without a warning',async()=>{
  for(const initial of [null,article]){const v=await mount(initial);try{if(initial){await act(async()=>edit(v,'English title','input','changed'));await act(async()=>edit(v,'English title','input','Saved title'));}await act(async()=>button(v,'Back to articles').props.onClick());assert.equal(v.root.findAllByProps({role:'alertdialog'}).length,0);assert.equal(v.root.findAllByType(ArticleEditor).length,0);}finally{act(()=>v.unmount());}}
});
test('a pending save prevents discard/unmount; failure retains the dirty draft',async()=>{
  const old=globalThis.fetch;let reject!:(e:Error)=>void;globalThis.fetch=async()=>new Promise<Response>((_r,j)=>{reject=j;});const v=await mount();
  try{
    await act(async()=>edit(v,'English title','input','Keep this draft'));
    let saving:Promise<void>|undefined;act(()=>{saving=button(v,'Save draft').props.onClick();});
    await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(button(v,'Discard and Leave').props.disabled,true);
    await act(async()=>button(v,'Discard and Leave').props.onClick());assert.equal(v.root.findAllByType(ArticleEditor).length,1);
    await act(async()=>{reject(new Error('Synthetic save failure'));await saving;});assert.equal(button(v,'Discard and Leave').props.disabled,false);
    await act(async()=>button(v,'Stay').props.onClick());assert.ok(JSON.stringify(v.toJSON()).includes('Keep this draft'));
    await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(v.root.findAllByProps({role:'alertdialog'}).length,1);
  } finally {act(()=>v.unmount());globalThis.fetch=old;}
});

for(const source of ['inclusion','questions']){
  test(`unsaved MCQ ${source} participates in the article navigation guard`,async()=>{
    const old=globalThis.fetch;globalThis.fetch=async()=>new Response(JSON.stringify({success:true,data:{schema_version:1,draft:null,published:null}}));const v=await mount();
    try{
      await act(async()=>button(v,'Open MCQ Builder').props.onClick());
      await act(async()=>{if(source==='questions')button(v,'Add question').props.onClick();else v.root.findAllByProps({className:'cms-quiz'})[0].findAllByType('input')[0].props.onChange({target:{checked:true}});});
      await act(async()=>button(v,'Sidebar destination').props.onClick());assert.equal(v.root.findAllByProps({role:'alertdialog'}).length,1);await act(async()=>button(v,'Stay').props.onClick());assert.equal(v.root.findAllByType(ArticleEditor).length,1);
    }finally{act(()=>v.unmount());globalThis.fetch=old;}
  });
}
