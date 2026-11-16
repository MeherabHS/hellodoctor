import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {QuizImport} from '../src/components/blogs/QuizImport';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';
import {newQuestion,quizTemplate,type QuizContent,type QuizRevision} from '../src/lib/quizContent';

const article={...blankArticle(),id:'00000000-0000-4000-8000-000000000001',status:'PUBLISHED',version:1,content_version:1} as AdminArticle;
function content():QuizContent {const q=newQuestion();q.text_en='Saved question';return {include_quiz:false,questions:[q]};}
function revision(c=content()):QuizRevision{return {...c,id:'00000000-0000-4000-8000-000000000002',article_id:article.id,status:'DRAFT',version:1,content_version:1,article_content_version:1,can_approve:false,requires_independent_review:false,approved_by:null};}
function button(view:ReactTestRenderer,name:string){return view.root.findAllByType('button').find(b=>b.children.join('')===name)!;}
const response=(data:unknown)=>new Response(JSON.stringify({success:true,data}));
function deferred<T>(){let resolve!:(v:T)=>void;const promise=new Promise<T>(r=>{resolve=r;});return {promise,resolve};}

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

