import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {McqBuilder} from '../src/components/blogs/McqBuilder';
import {QuizImport} from '../src/components/blogs/QuizImport';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';
import {newQuestion,quizTemplate,type QuizContent,type QuizRevision} from '../src/lib/quizContent';

const article={...blankArticle(),id:'00000000-0000-4000-8000-000000000001',status:'PUBLISHED',version:1,content_version:1} as AdminArticle;
function content():QuizContent {const q=newQuestion();q.text_en='Saved question';return {include_quiz:false,questions:[q]};}
function revision(c=content()):QuizRevision{return {...c,id:'00000000-0000-4000-8000-000000000002',article_id:article.id,status:'DRAFT',version:1,content_version:1,article_content_version:1,can_approve:false,requires_independent_review:false,approved_by:null};}
function button(view:ReactTestRenderer,name:string){return view.root.findAllByType('button').find(b=>b.children.join('')===name)!;}
const response=(data:unknown)=>new Response(JSON.stringify({success:true,data}));
function deferred<T>(){let resolve!:(v:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve};}

test('delayed import cannot overwrite newer question edits or clear their dirty state',async()=>{
  const previous=globalThis.fetch, waiting=deferred<Response>();const original=revision();let calls=0;const dirty:boolean[]=[];let view:ReactTestRenderer|undefined;
  globalThis.fetch=async(url)=>{if(String(url).endsWith('/import-preview'))return response({questions:original.questions,errors:[]});if(String(url).endsWith('/import')){calls++;return waiting.promise;}return response({schema_version:1,draft:original,published:null});};
  try{
    await act(async()=>{view=create(<McqBuilder article={article} onDirty={v=>dirty.push(v)} onPreview={()=>{}}/>);});
    await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});
    await act(async()=>{view!.root.findAllByType('input').find(n=>n.props.type==='file')!.props.onChange({target:{files:[new File([quizTemplate('json')],'questions.json')]}});});
    await act(async()=>{button(view!,'Preview import').props.onClick();});
    let operation:Promise<void>|undefined;
    act(()=>{operation=button(view!,'Confirm import to draft').props.onClick();});
    await act(async()=>{view!.root.findAllByType('textarea')[0].props.onChange({target:{value:'Newer local edit'}});});
    const editingBlocked=view!.root.findAllByType('textarea')[0].props.value==='Saved question'&&view!.root.findAllByType('fieldset')[0].props.disabled===true;
    await act(async()=>{waiting.resolve(response({schema_version:1,draft:{...original,version:2},published:null}));await operation;});
    assert.equal(calls,1);
    const shown=view!.root.findAllByType('textarea')[0].props.value;
    // A shared gate may reject the edit while import owns the mutation; a
    // generation-aware strategy may preserve it as dirty. Silent loss is invalid.
    assert.ok(shown==='Newer local edit'&&dirty.at(-1)===true || editingBlocked);
  } finally {if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});

for(const stage of ['preview','confirm','unmount'] as const){
  test(`${stage}: obsolete import response is ignored after revision change or unmount`,async()=>{
    const previous=globalThis.fetch,waiting=deferred<Response>(),original=revision();let imported=0;let view:ReactTestRenderer|undefined;
    globalThis.fetch=async(url)=>String(url).endsWith('/import-preview')&&stage!=='preview'?response({questions:original.questions,errors:[]}):waiting.promise;
    const render=(r:QuizRevision)=><QuizImport articleId={article.id} revision={r} disabled={false} onImported={()=>{imported++;}}/>;
    try{
      await act(async()=>{view=create(render(original));});await act(async()=>{view!.root.findByType('input').props.onChange({target:{files:[new File([quizTemplate('json')],'questions.json')]}});});
      let work:Promise<void>|undefined;
      if(stage==='preview')act(()=>{work=button(view!,'Preview import').props.onClick();});
      else{await act(async()=>{button(view!,'Preview import').props.onClick();});act(()=>{work=button(view!,'Confirm import to draft').props.onClick();});}
      await act(async()=>{if(stage==='unmount'){view!.unmount();}else view!.update(render({...original,version:2}));});
      await act(async()=>{waiting.resolve(response(stage==='preview'?{questions:original.questions,errors:[]}:{schema_version:1,draft:original,published:null}));await work;});
      assert.equal(imported,0);if(stage!=='unmount')assert.ok(!view!.root.findAllByType('button').some(b=>b.children.join('')==='Confirm import to draft'));
    }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
  });
}

test('failed import preserves questions and unsaved import, then permits manual edits',async()=>{
  const previous=globalThis.fetch,original=revision();const dirty:boolean[]=[];let view:ReactTestRenderer|undefined;
  globalThis.fetch=async(url)=>String(url).endsWith('/import-preview')?response({questions:original.questions,errors:[]}):String(url).endsWith('/import')?new Response(JSON.stringify({success:false,error:'Synthetic import unavailable'}),{status:503}):response({schema_version:1,draft:original,published:null});
  try{
    await act(async()=>{view=create(<McqBuilder article={article} onDirty={v=>dirty.push(v)} onPreview={()=>{}}/>);});await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});
    await act(async()=>{view!.root.findAllByType('input').find(n=>n.props.type==='file')!.props.onChange({target:{files:[new File([quizTemplate('json')],'questions.json')]}});});
    assert.equal(dirty.at(-1),true);await act(async()=>{button(view!,'Preview import').props.onClick();});await act(async()=>{button(view!,'Confirm import to draft').props.onClick();});
    assert.equal(view!.root.findAllByType('textarea')[0].props.value,'Saved question');assert.equal(dirty.at(-1),true);
    await act(async()=>{view!.root.findAllByType('textarea')[0].props.onChange({target:{value:'Kept after failure'}});});assert.equal(view!.root.findAllByType('textarea')[0].props.value,'Kept after failure');assert.equal(dirty.at(-1),true);
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});

test('shared import gate prevents duplicate confirmation and parallel save/add/inclusion changes',async()=>{
  const previous=globalThis.fetch,waiting=deferred<Response>(),original=revision();let writes=0;let view:ReactTestRenderer|undefined;const dirty:boolean[]=[];
  globalThis.fetch=async(url,init)=>{if(String(url).endsWith('/import-preview'))return response({questions:original.questions,errors:[]});if(init?.method){writes++;return waiting.promise;}return response({schema_version:1,draft:original,published:null});};
  try{
    await act(async()=>{view=create(<McqBuilder article={article} onDirty={v=>dirty.push(v)} onPreview={()=>{}}/>);});await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});
    await act(async()=>{view!.root.findAllByType('input').find(n=>n.props.type==='file')!.props.onChange({target:{files:[new File([quizTemplate('json')],'questions.json')]}});});await act(async()=>{button(view!,'Preview import').props.onClick();});
    let first:Promise<void>|undefined;act(()=>{first=button(view!,'Confirm import to draft').props.onClick();button(view!,'Confirm import to draft').props.onClick();button(view!,'Save question draft').props.onClick();button(view!,'Add question').props.onClick();view!.root.findAllByType('input').find(n=>n.props.type==='checkbox')!.props.onChange({target:{checked:true}});});
    assert.equal(writes,1);assert.equal(view!.root.findAllByType('input').find(n=>n.props.type==='checkbox')!.props.checked,false);
    const imported={...original,version:2,questions:[...original.questions,newQuestion()]};await act(async()=>{waiting.resolve(response({schema_version:1,draft:imported,published:null}));await first;});
    assert.equal(view!.root.findAllByProps({className:'cms-question'}).length,2);assert.equal(dirty.at(-1),false);assert.equal(writes,1);
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});
