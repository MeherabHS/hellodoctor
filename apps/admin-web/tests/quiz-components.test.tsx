import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {renderToStaticMarkup} from 'react-dom/server';
import {McqBuilder} from '../src/components/blogs/McqBuilder';
import {QuestionEditor} from '../src/components/blogs/QuestionEditor';
import {QuizPreview} from '../src/components/blogs/QuizPreview';
import {QuizImport} from '../src/components/blogs/QuizImport';
import {ArticlePreview} from '../src/components/blogs/ArticlePreview';
import {newQuestion,editableQuiz,quizTemplate,type QuizContent,type QuizDocument,type QuizRevision} from '../src/lib/quizContent';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';
const article={...blankArticle(),id:'00000000-0000-4000-8000-000000000001',author_id:'author',title_en:'Article body',body_en:'Article paragraph',status:'PUBLISHED',version:4,content_version:1} as AdminArticle;
function ready():QuizContent{const q=newQuestion();q.text_en='Which answer?';q.text_sw='Jibu gani?';q.explanation_en='First is correct';q.explanation_sw='Kwanza ni sahihi';q.options[0].text_en='First';q.options[0].text_sw='Kwanza';q.options[1].text_en='Second';q.options[1].text_sw='Pili';q.correct_option_id=q.options[0].id;return {include_quiz:true,questions:[q]};}
function revision(content:QuizContent=ready()):QuizRevision{return {...content,id:'00000000-0000-4000-8000-000000000002',article_id:article.id,status:'DRAFT',version:1,content_version:1,article_content_version:1,can_approve:false,requires_independent_review:true,approved_by:null};}
function button(view:ReactTestRenderer,name:string){return view.root.findAllByType('button').find(b=>b.children.join('')===name)!;}
function response(data:unknown){return new Response(JSON.stringify({success:true,data}));}

test('manual question controls preserve IDs, support translation, options and correct selection',async()=>{
  const q=ready().questions[0];let updated=q;let view:ReactTestRenderer|undefined;
  try{await act(async()=>{view=create(<QuestionEditor question={q} index={0} total={1} language="sw" onChange={v=>{updated=v;}} onRemove={()=>{}} onMove={()=>{}}/>);});
    const radio=view!.root.findAllByType('input').filter(i=>i.props.type==='radio')[1];await act(async()=>{radio.props.onChange();});assert.equal(updated.correct_option_id,q.options[1].id);
    assert.equal(view!.root.findAllByType('textarea')[0].props.value,'Jibu gani?');await act(async()=>{button(view!,'Add answer option').props.onClick();});assert.equal(updated.options.length,3);assert.equal(updated.id,q.id);
  }finally{if(view)act(()=>view!.unmount());}
});

test('builder loads on demand, edits locally, saves excluded incomplete drafts and keeps review separate',async()=>{
  const previous=globalThis.fetch;const requests:{url:string;method:string;body:Record<string,unknown>|null}[]=[];const previews:QuizContent[]=[];const dirty:boolean[]=[];
  globalThis.fetch=async(url,init)=>{const body=init?.body?JSON.parse(String(init.body)):null;requests.push({url:String(url),method:init?.method||'GET',body});return response(body?{schema_version:1,draft:revision(body.content as QuizContent),published:null}:{schema_version:1,draft:null,published:null});};
  let view:ReactTestRenderer|undefined;
  try{await act(async()=>{view=create(<McqBuilder article={article} onPreview={c=>{if(c)previews.push(c);}} onDirty={d=>dirty.push(d)}/>);});assert.equal(requests.length,0);
    await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});assert.equal(requests.length,1);
    const checkbox=view!.root.findAllByType('input').find(i=>i.props.type==='checkbox')!;assert.equal(checkbox.props.checked,false);
    await act(async()=>{button(view!,'Add question').props.onClick();});await act(async()=>{button(view!,'Kiswahili questions').props.onClick();});
    assert.equal(requests.length,1);assert.equal(previews.at(-1)!.questions.length,1);assert.equal(previews.at(-1)!.include_quiz,false);
    await act(async()=>{button(view!,'Save question draft').props.onClick();});assert.equal(requests.length,2);assert.equal(requests[1].method,'PATCH');assert.equal((requests[1].body!.content as QuizContent).include_quiz,false);assert.equal(dirty.at(-1),false);
    assert.ok(JSON.stringify(view!.toJSON()).includes('Published article and quiz remain unchanged.'));assert.equal(button(view!,'Submit quiz for review').props.disabled,false);
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});

test('sensitive quiz author sees disabled approval; server rejection remains readable',async()=>{
  const previous=globalThis.fetch;let writes=0;let view:ReactTestRenderer|undefined;
  const pending={...revision(),status:'PENDING_REVIEW' as const};globalThis.fetch=async()=>response({schema_version:1,draft:pending,published:null});
  try{await act(async()=>{view=create(<McqBuilder article={article} onPreview={()=>{}} onDirty={()=>{}}/>);});await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});assert.equal(button(view!,'Approve quiz').props.disabled,true);assert.ok(JSON.stringify(view!.toJSON()).includes('Approval required from another Admin.'));
    act(()=>view!.unmount());pending.can_approve=true;
    globalThis.fetch=async(_url,init)=>{if(!init?.method)return response({schema_version:1,draft:pending,published:null});writes++;return new Response(JSON.stringify({success:false,error:{message:'Approval required from another Admin.'}}),{status:403});};
    await act(async()=>{view=create(<McqBuilder article={article} onPreview={()=>{}} onDirty={()=>{}}/>);});await act(async()=>{button(view!,'Open MCQ Builder').props.onClick();});await act(async()=>{button(view!,'Approve quiz').props.onClick();});assert.equal(writes,1);const output=JSON.stringify(view!.toJSON());assert.ok(output.includes('Approval required from another Admin.'));assert.ok(!output.includes('API Error'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});

test('interactive preview gives local feedback, navigation, progress and completion without transport',async()=>{
  const previous=globalThis.fetch;let calls=0;globalThis.fetch=async()=>{calls++;throw new Error('Preview must not call any API');};let view:ReactTestRenderer|undefined;
  const content=ready();const second=ready().questions[0];second.text_en='Next question';content.questions.push(second);
  try{await act(async()=>{view=create(<QuizPreview content={content} language="en"/>);});
    await act(async()=>{view!.root.findAllByProps({role:'radio'})[1].props.onClick();});await act(async()=>{button(view!,'Check answer').props.onClick();});assert.ok(JSON.stringify(view!.toJSON()).includes('Not quite'));assert.ok(JSON.stringify(view!.toJSON()).includes('First is correct'));
    await act(async()=>{button(view!,'Next question').props.onClick();});assert.ok(JSON.stringify(view!.toJSON()).includes('Next question'));
    await act(async()=>{view!.root.findAllByProps({role:'radio'})[0].props.onClick();});await act(async()=>{button(view!,'Check answer').props.onClick();});assert.ok(JSON.stringify(view!.toJSON()).includes('Correct'));
    await act(async()=>{button(view!,'Finish preview').props.onClick();});assert.ok(JSON.stringify(view!.toJSON()).includes('Preview complete'));assert.equal(calls,0);
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});

test('article preview places the optional quiz below content and escapes untrusted bilingual text',()=>{
  const content=ready();content.questions[0].text_en='<script>alert(1)</script>';let html=renderToStaticMarkup(<ArticlePreview content={article} article={article} language="en" setLanguage={()=>{}} localCover="" quiz={content}/>);
  assert.ok(!html.includes('<script>'));assert.ok(html.includes('&lt;script&gt;'));assert.ok(html.indexOf('Article paragraph')<html.indexOf('Check your understanding'));
  html=renderToStaticMarkup(<QuizPreview content={content} language="sw"/>);assert.ok(html.includes('Jibu gani?'));
  assert.equal(renderToStaticMarkup(<QuizPreview content={{...content,include_quiz:false}} language="en"/>),'');
});

test('import preview shows row validation and requires explicit confirmation before saving',async()=>{
  const previous=globalThis.fetch;const calls:string[]=[];let imported:QuizDocument|null=null;let valid=false;let view:ReactTestRenderer|undefined;
  const q=ready().questions[0];globalThis.fetch=async(url)=>{calls.push(String(url));return response(String(url).endsWith('/import-preview')?{questions:[q],errors:valid?[]:[{row:2,field:'correct_option',message:'Select a valid choice.'}]}:{schema_version:1,draft:revision(),published:null});};
  try{await act(async()=>{view=create(<QuizImport articleId={article.id} revision={null} disabled={false} onImported={d=>{imported=d;}}/>);});
    const select=async()=>{await act(async()=>{view!.root.findByType('input').props.onChange({target:{files:[new File([quizTemplate('json')],'questions.json')]}});});};
    await select();assert.equal(calls.length,0);await act(async()=>{button(view!,'Preview import').props.onClick();});assert.ok(JSON.stringify(view!.toJSON()).includes('Select a valid choice.'));assert.equal(button(view!,'Confirm import to draft').props.disabled,true);assert.equal(imported,null);
    valid=true;await select();await act(async()=>{button(view!,'Preview import').props.onClick();});assert.equal(calls.filter(url=>url.endsWith('/import')).length,0);
    await act(async()=>{button(view!,'Confirm import to draft').props.onClick();});assert.ok(imported);assert.equal(calls.filter(url=>url.endsWith('/import')).length,1);assert.ok(calls.every(url=>!url.includes('publish')&&!url.includes('score')&&!url.includes('attempt')));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=previous;}
});
