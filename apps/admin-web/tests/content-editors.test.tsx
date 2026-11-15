import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {NavigationGuard} from '../src/components/navigation/NavigationGuard';
import {VideoEditor} from '../src/components/blogs/VideoEditor';
import {QuizEditor} from '../src/components/blogs/QuizEditor';
import {categoryFor,youTubeId,OTHER} from '../src/lib/contentTopics';
import {blankArticle,type AdminArticle,type ArticleCategory} from '../src/lib/blogContent';

const cat=(over:Partial<ArticleCategory>):ArticleCategory=>({id:'x',name_en:'X',name_sw:'X',sensitive:true,topic_key:null,...over});

test('a topic reuses its category, or creates one with the Patient app Kiswahili name',async()=>{
  const saved:unknown[]=[];const save=async(c:Omit<ArticleCategory,'id'>,id?:string)=>{saved.push({...c,id});return {...c,id:id??'new'};};
  assert.deepEqual(await categoryFor({topic:'PREP',other:''},[cat({id:'p',topic_key:'PREP'})],save),{id:'p',created:false});
  assert.equal(saved.length,0);
  assert.deepEqual(await categoryFor({topic:'CONDOMS',other:''},[],save),{id:'new',created:true});
  assert.deepEqual(saved.pop(),{name_en:'Condoms',name_sw:'Kondomu',sensitive:true,topic_key:'CONDOMS',id:undefined});
  // A same-named category without a topic is linked rather than duplicated (names are unique).
  await categoryFor({topic:'HIV_BASICS',other:''},[cat({id:'b',name_en:'HIV Basics',name_sw:'Msingi wa VVU'})],save);
  assert.deepEqual(saved.pop(),{name_en:'HIV Basics',name_sw:'Msingi wa VVU',sensitive:true,topic_key:'HIV_BASICS',id:'b'});
});

test('"Other" uses the typed name: an existing category or a new one with no topic',async()=>{
  const saved:Omit<ArticleCategory,'id'>[]=[];const save=async(c:Omit<ArticleCategory,'id'>)=>{saved.push(c);return {...c,id:'o'};};
  assert.deepEqual(await categoryFor({topic:OTHER,other:'  mental health '},[cat({id:'m',name_en:'Mental Health'})],save),{id:'m',created:false});
  await categoryFor({topic:OTHER,other:'Nutrition'},[],save);
  assert.deepEqual(saved,[{name_en:'Nutrition',name_sw:'Nutrition',sensitive:true,topic_key:null}]);
  await assert.rejects(categoryFor({topic:OTHER,other:' '},[],save),/topic name/);
  await assert.rejects(categoryFor({topic:'',other:''},[],save),/Choose a topic/);
});

test('YouTube ids are read from every link shape the backend accepts',()=>{
  for(const link of ['https://www.youtube.com/watch?v=dQw4w9WgXcQ','https://youtu.be/dQw4w9WgXcQ?si=x','https://m.youtube.com/watch?feature=a&v=dQw4w9WgXcQ','https://www.youtube.com/shorts/dQw4w9WgXcQ'])
    assert.equal(youTubeId(link),'dQw4w9WgXcQ',link);
  for(const bad of ['https://evil.example/watch?v=dQw4w9WgXcQ','dQw4w9WgXcQ',''])assert.equal(youTubeId(bad),null,bad);
});

// A fake backend: records requests and plays the article/quiz state machine.
function server(){
  const calls:{method:string;path:string;body:Record<string,unknown>|null}[]=[];
  let article:Record<string,unknown>|null=null;let quiz:Record<string,unknown>|null=null;let version=0;
  const ok=(data:unknown)=>new Response(JSON.stringify({success:true,data}),{headers:{'Content-Type':'application/json'}});
  const fetch=async(url:RequestInfo|URL,init?:RequestInit)=>{
    const path=String(url).replace('/api/proxy','').split('?')[0];const method=init?.method||'GET';
    const body=init?.body&&typeof init.body==='string'?JSON.parse(init.body):null;calls.push({method,path,body});
    if(path.startsWith('/api/youtube-thumbnail/'))return new Response(new Uint8Array([255,216,255]),{headers:{'Content-Type':'image/jpeg'}});
    if(path.endsWith('/admin/article-categories'))return ok(method==='POST'?{...body,id:'cat-new'}:[]);
    if(path.startsWith('/api/article-media/')){article={...article,cover_image_id:'cover',version:++version};return ok(article);}
    if(path.endsWith('/quiz')){quiz={...(body!.content as object),id:'rev',article_id:article!.id,status:'DRAFT',version:1,content_version:1,article_content_version:article!.content_version};return ok({schema_version:1,draft:quiz,published:null});}
    const quizStep=path.match(/\/quiz\/rev\/(submit|approve|publish)$/);
    if(quizStep){quiz={...quiz,status:{submit:'PENDING_REVIEW',approve:'APPROVED',publish:'PUBLISHED'}[quizStep[1]]};return ok({schema_version:1,draft:quizStep[1]==='publish'?null:quiz,published:quizStep[1]==='publish'?quiz:null});}
    const step=path.match(/\/(submit|approve|publish)$/);
    if(step){article={...article,status:{submit:'PENDING_REVIEW',approve:'APPROVED',publish:'PUBLISHED'}[step[1]],version:++version};return ok(article);}
    if(path.endsWith('/admin/articles')&&method==='POST'){article={...blankArticle(),...(body!.content as object),id:body!.id,status:'DRAFT',version:++version,content_version:1,cover_image_id:null};return ok(article);}
    throw new Error(`unexpected ${method} ${path}`);
  };
  return {calls,fetch:fetch as typeof globalThis.fetch};
}
const by=(v:ReactTestRenderer,id:string)=>v.root.find(n=>n.props.id===id&&['input','textarea'].includes(String(n.type)));
const button=(v:ReactTestRenderer,label:string)=>v.root.findAllByType('button').find(b=>b.children.join('')===label)!;
async function withEnv<T>(fake:ReturnType<typeof server>,work:()=>Promise<T>){
  const before={fetch:globalThis.fetch,window:globalThis.window,reader:globalThis.FileReader};
  globalThis.fetch=fake.fetch;globalThis.window={addEventListener(){},removeEventListener(){},confirm(){return true;}} as unknown as Window&typeof globalThis;
  globalThis.FileReader=class{result='data:image/jpeg;base64,AA==';onload:(()=>void)|null=null;readAsDataURL(){this.onload?.();}abort(){}} as unknown as typeof FileReader;
  try{return await work();}finally{globalThis.fetch=before.fetch;globalThis.window=before.window;globalThis.FileReader=before.reader;}
}

test('a video: the YouTube cover is fetched from the link, then one Publish saves and publishes it',async()=>{
  const fake=server();let closed:string|undefined;let v!:ReactTestRenderer;
  await withEnv(fake,async()=>{
    await act(async()=>{v=create(<NavigationGuard><VideoEditor initial={null} categories={[]} onSaved={()=>{}} onClose={m=>{closed=m;}}/></NavigationGuard>);});
    assert.equal(button(v,'Publish').props.disabled,true,'nothing to publish yet');
    await act(async()=>{by(v,'video-title').props.onChange({target:{value:'How PrEP works'}});});
    await act(async()=>{by(v,'video-sub').props.onChange({target:{value:'A two-minute explainer.'}});});
    await act(async()=>{by(v,'video-link').props.onChange({target:{value:'https://youtu.be/dQw4w9WgXcQ'}});});
    assert.ok(fake.calls.some(c=>c.path==='/api/youtube-thumbnail/dQw4w9WgXcQ'),'cover fetched from YouTube');
    await act(async()=>{button(v,'Publish').props.onClick();});
    const order=fake.calls.filter(c=>!c.path.startsWith('/api/youtube')).map(c=>`${c.method} ${c.path.replace(/[0-9a-f-]{36}/,'ID')}`);
    assert.deepEqual(order,['GET /admin/article-categories','POST /admin/article-categories','POST /admin/articles','POST /api/article-media/ID','POST /admin/articles/ID/submit','POST /admin/articles/ID/approve','POST /admin/articles/ID/publish']);
    assert.deepEqual(fake.calls.find(c=>c.method==='POST'&&c.path.endsWith('/admin/article-categories'))!.body,{name_en:'Videos',name_sw:'Video',sensitive:true,topic_key:null});
    const saved=fake.calls.find(c=>c.path==='/admin/articles')!.body!.content as Record<string,string>;
    assert.equal(saved.video_url,'https://youtu.be/dQw4w9WgXcQ');assert.equal(saved.category_id,'cat-new');
    assert.equal(closed,'Published “How PrEP works”.');
    act(()=>v.unmount());
  });
});

function fillQuestion(v:ReactTestRenderer){
  const q=v.root.findAll(n=>n.type==='section'&&n.props.className==='cms-question')[0];
  const texts=q.findAllByType('textarea');const inputs=q.findAll(n=>n.type==='input'&&n.props.className==='afya-input');
  texts[0].props.onChange({target:{value:'What does PrEP prevent?'}});
  return {q,texts,inputs};
}

test('a quiz: header, sub header and questions are published together, entry first',async()=>{
  const fake=server();let closed:string|undefined;let v!:ReactTestRenderer;
  await withEnv(fake,async()=>{
    await act(async()=>{v=create(<NavigationGuard><QuizEditor initial={null} categories={[]} onSaved={()=>{}} onClose={m=>{closed=m;}}/></NavigationGuard>);});
    await act(async()=>{by(v,'quiz-title').props.onChange({target:{value:'PrEP check'}});});
    await act(async()=>{by(v,'quiz-sub').props.onChange({target:{value:'Five quick questions.'}});});
    assert.equal(button(v,'Publish').props.disabled,true,'an unfinished question blocks Publish');
    await act(async()=>{fillQuestion(v);});
    await act(async()=>{const {inputs}=fillQuestion(v);inputs[0].props.onChange({target:{value:'HIV'}});});
    await act(async()=>{const {inputs}=fillQuestion(v);inputs[1].props.onChange({target:{value:'Flu'}});});
    await act(async()=>{const {q}=fillQuestion(v);q.findAll(n=>n.type==='input'&&n.props.type==='radio')[0].props.onChange();});
    await act(async()=>{const {texts}=fillQuestion(v);texts[1].props.onChange({target:{value:'PrEP prevents HIV.'}});});
    assert.equal(button(v,'Publish').props.disabled,false);
    await act(async()=>{button(v,'Publish').props.onClick();});
    const order=fake.calls.map(c=>`${c.method} ${c.path.replace(/[0-9a-f-]{36}/,'ID')}`);
    assert.deepEqual(order,['GET /admin/article-categories','POST /admin/article-categories','POST /admin/articles','PATCH /admin/articles/ID/quiz',
      'POST /admin/articles/ID/submit','POST /admin/articles/ID/approve','POST /admin/articles/ID/publish',
      'POST /admin/articles/ID/quiz/rev/submit','POST /admin/articles/ID/quiz/rev/approve','POST /admin/articles/ID/quiz/rev/publish']);
    assert.equal(fake.calls[2].body!.standalone_quiz,true);
    assert.equal((fake.calls[3].body!.content as {include_quiz:boolean}).include_quiz,true);
    assert.equal(closed,'Published “PrEP check”.');
    act(()=>v.unmount());
  });
});

test('a quiz draft with an unfinished question saves without including it',async()=>{
  const fake=server();let v!:ReactTestRenderer;
  await withEnv(fake,async()=>{
    await act(async()=>{v=create(<NavigationGuard><QuizEditor initial={null} categories={[]} onSaved={()=>{}} onClose={()=>{}}/></NavigationGuard>);});
    await act(async()=>{by(v,'quiz-title').props.onChange({target:{value:'Draft quiz'}});});
    await act(async()=>{by(v,'quiz-sub').props.onChange({target:{value:'Not finished.'}});});
    await act(async()=>{fillQuestion(v);});
    await act(async()=>{button(v,'Save draft').props.onClick();});
    const quizSave=fake.calls.find(c=>c.path.endsWith('/quiz'))!;
    assert.equal((quizSave.body!.content as {include_quiz:boolean}).include_quiz,false);
    assert.ok(!fake.calls.some(c=>c.path.endsWith('/publish')),'nothing published');
    assert.ok(JSON.stringify(v.toJSON()).includes('Draft saved'));
    act(()=>v.unmount());
  });
});

test('an existing published quiz offers Update, never a Save draft that would unpublish it',async()=>{
  const published={...blankArticle(),id:'00000000-0000-4000-8000-0000000000aa',title_en:'Live quiz',body_en:'Intro',category_id:'q',status:'PUBLISHED',version:3,content_version:1,cover_image_id:null,standalone_quiz:true} as AdminArticle;
  const fake=server();let v!:ReactTestRenderer;
  await withEnv({...fake,fetch:(async()=>new Response(JSON.stringify({success:true,data:{schema_version:1,draft:null,published:null}}))) as typeof fetch},async()=>{
    await act(async()=>{v=create(<NavigationGuard><QuizEditor initial={published} categories={[]} onSaved={()=>{}} onClose={()=>{}}/></NavigationGuard>);});
    const labels=v.root.findAllByType('button').map(b=>b.children.join(''));
    assert.ok(labels.includes('Update'));assert.ok(!labels.includes('Save draft'));
    act(()=>v.unmount());
  });
});
