import React from 'react';
import test from 'node:test';
import assert from 'node:assert/strict';
import {renderToStaticMarkup} from 'react-dom/server';
import {act,create,type ReactTestRenderer} from 'react-test-renderer';
import {SafeMarkdown} from '../src/components/blogs/SafeMarkdown';
import {ArticlePreview} from '../src/components/blogs/ArticlePreview';
import {ArticleEditor as Editor} from '../src/components/blogs/ArticleEditor';
import {NavigationGuard} from '../src/components/navigation/NavigationGuard';
import {blankArticle,type AdminArticle} from '../src/lib/blogContent';
const content={...blankArticle(),title_en:'Health guide',body_en:'## A heading\n\nA **strong** paragraph.\n\n- First\n- Second\n\n> Quoted\n\n[Read more](https://example.org)',category_id:'c'};
const category={id:'c',name_en:'Health',name_sw:'Afya',sensitive:true};
function ArticleEditor(props:React.ComponentProps<typeof Editor>){return <NavigationGuard><Editor {...props}/></NavigationGuard>;}
const article:AdminArticle={...content,id:'e0f20dc7-61b7-432e-805c-ded77ab2ee78',author_id:'author',author_name:'Editorial',status:'DRAFT',version:1,content_version:1,approved_version:null,approved_by:null,approved_at:null,requires_independent_review:true,can_approve:false,approval_block_reason:null,cover_image_id:null,created_at:'2026-10-25T10:00:00Z',updated_at:'2026-10-25T10:00:00Z',published_at:null,scheduled_for:null};
test('Markdown renders supported formatting and disables HTML, scripts and embedded media',()=>{
  const html=renderToStaticMarkup(<SafeMarkdown body={`${content.body_en}\n\n<script>alert(1)</script>\n\n<img src=x onerror=alert(1)>\n\n![track](https://tracking.example/x)\n\n[bad](javascript:alert(1))`}/>);
  for(const tag of ['<h2>','<strong>','<ul>','<blockquote>','href="https://example.org/"'])assert.ok(html.includes(tag),tag);
  for(const unsafe of ['<script','<img','javascript:','onerror='])assert.ok(!html.includes(unsafe),unsafe);
});

// Admins publish directly (CLAUDE.md, 2026-11-14): no Submit for review,
// Approve or Return controls; Publish runs the backend's remaining internal
// steps from whatever state the article is in.
function stubWindow(){const previous=globalThis.window;globalThis.window={addEventListener(){},removeEventListener(){},confirm(){return true;}} as unknown as Window & typeof globalThis;return previous;}
function stepServer(paths:string[],fail?:(path:string)=>Response|undefined){
  let version=1;
  return async(url:RequestInfo|URL)=>{const path=String(url);paths.push(path);const failed=fail?.(path);if(failed)return failed;version++;
    const status=path.endsWith('/submit')?'PENDING_REVIEW':path.endsWith('/approve')?'APPROVED':path.endsWith('/publish')?'PUBLISHED':'DRAFT';
    return new Response(JSON.stringify({success:true,data:{...article,status,version}}));};
}
test('a sensitive draft is published directly; the review steps run behind Publish',async()=>{
  const previous=stubWindow();const fetchBefore=globalThis.fetch;const paths:string[]=[];globalThis.fetch=stepServer(paths) as typeof fetch;
  let view:ReactTestRenderer|undefined;
  try{
    let closed:string|undefined;
    await act(async()=>{view=create(<ArticleEditor initial={article} categories={[category]} onSaved={()=>{}} onClose={m=>{closed=m;}}/>);});
    const labels=view!.root.findAllByType('button').map(b=>b.children.join(''));
    for(const removed of ['Approve','Submit for review','Return'])assert.ok(!labels.includes(removed),removed);
    await act(async()=>{view!.root.findAllByType('button').find(b=>b.children.join('')==='Publish')!.props.onClick();});
    assert.deepEqual(paths.map(p=>p.split('/').pop()),['submit','approve','publish']);
    assert.equal(closed,'Published “Health guide”.','publishing returns to the list');
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('an article left pending review is finished by the next Publish',async()=>{
  const previous=stubWindow();const fetchBefore=globalThis.fetch;const paths:string[]=[];globalThis.fetch=stepServer(paths) as typeof fetch;
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={{...article,status:'PENDING_REVIEW'}} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    await act(async()=>{view!.root.findAllByType('button').find(b=>b.children.join('')==='Publish')!.props.onClick();});
    assert.deepEqual(paths.map(p=>p.split('/').pop()),['approve','publish']);
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('a server refusal while publishing stays readable and keeps the article unpublished',async()=>{
  const previous=stubWindow();const fetchBefore=globalThis.fetch;const paths:string[]=[];
  globalThis.fetch=stepServer(paths,path=>path.endsWith('/approve')?new Response(JSON.stringify({success:false,error:{code:'FORBIDDEN',message:'You do not have permission to publish this article.'}}),{status:403}):undefined) as typeof fetch;
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={article} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    await act(async()=>{view!.root.findAllByType('button').find(b=>b.children.join('')==='Publish')!.props.onClick();});
    const output=JSON.stringify(view!.toJSON());
    assert.ok(output.includes('Not published'));assert.ok(output.includes('permission'));assert.ok(!output.includes('API Error'));assert.ok(!output.includes('FORBIDDEN'));
    assert.ok(!paths.some(p=>p.endsWith('/publish')));
    assert.ok(view!.root.findAllByType('button').some(b=>b.children.join('')==='Publish'),'Publish stays available to retry');
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('failed cover upload preserves the saved draft and selection without reporting success',async()=>{
  const windowBefore=globalThis.window;const fetchBefore=globalThis.fetch;const readerBefore=globalThis.FileReader;
  globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window & typeof globalThis;
  globalThis.FileReader=class{result='data:image/png;base64,fixture';onload:(()=>void)|null=null;readAsDataURL(){this.onload?.();}abort(){}} as unknown as typeof FileReader;
  const requests:string[]=[];let refreshed=0;let view:ReactTestRenderer|undefined;
  globalThis.fetch=async(url)=>{requests.push(String(url));return new Response(JSON.stringify({success:false,error:{message:'Article media storage is not configured.'}}),{status:409});};
  try{
    await act(async()=>{view=create(<ArticleEditor initial={article} categories={[category]} onSaved={()=>{refreshed++;}} onClose={()=>{}}/>);});
    await act(async()=>{view!.root.findAllByType('input').find(input=>input.props.type==='file')!.props.onChange({target:{files:[new File(['fixture'],'cover.png',{type:'image/png'})]}});});
    await act(async()=>{view!.root.findAllByType('button').find(button=>button.children.join('')==='Save draft')!.props.onClick();});
    const output=JSON.stringify(view!.toJSON());
    // Unchanged text is not saved again; only the cover upload is sent.
    assert.equal(requests.length,1);assert.ok(requests[0].endsWith(`/${article.id}?version=1`));assert.equal(refreshed,1);
    assert.ok(output.includes('Text saved, but the image was not'));assert.ok(output.includes('awaiting Azure Blob integration'));
    assert.ok(view!.root.findAllByType('button').some(button=>button.props['aria-label']==='Remove selected image'),'the chosen image is kept to retry');
    assert.ok(!output.includes('Draft saved.'));
    assert.ok(!output.includes('API Error'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=windowBefore;globalThis.FileReader=readerBefore;}
});
test('live preview renders unsaved bilingual content without transport or publication actions',()=>{
  const previous=globalThis.fetch;let calls=0;globalThis.fetch=async()=>{calls++;throw new Error('Preview must not fetch');};
  try{const html=renderToStaticMarkup(<ArticlePreview content={{...content,title_sw:'Mwongozo',body_sw:'## Afya'}} category={category} language="sw" setLanguage={()=>{}} article={null} localCover=""/>);assert.ok(html.includes('Mwongozo'));assert.ok(html.includes('never publishes'));assert.ok(html.includes('AfyaMkononi Editorial'));assert.ok(!html.includes('Publish now'));assert.equal(calls,0);}finally{globalThis.fetch=previous;}
});
test('preview cover is same-origin authenticated media and shows clear fallback',()=>{
  const html=renderToStaticMarkup(<ArticlePreview content={content} category={category} language="sw" setLanguage={()=>{}} article={{...article,cover_image_id:'cover'}} localCover=""/>);
  assert.ok(html.includes(`/api/article-media/${article.id}`));assert.ok(html.includes('English fallback'));assert.ok(html.includes('Draft'));
});
test('editor typing and language switching remain local; draft save and publishing are explicit',async()=>{
  const fetchBefore=globalThis.fetch;const windowBefore=globalThis.window;
  const requests:{url:string;body:Record<string,unknown>}[]=[];
  globalThis.window={addEventListener(){},removeEventListener(){},confirm(){return true;}} as unknown as Window & typeof globalThis;
  globalThis.fetch=async(url,init)=>{requests.push({url:String(url),body:JSON.parse(String(init?.body))});return new Response(JSON.stringify({success:true,data:requests.length===1?article:{...article,status:'PENDING_REVIEW',version:2}}),{status:200,headers:{'Content-Type':'application/json'}});};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={article} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    const root=view!.root;
    await act(async()=>{root.findByType('textarea').props.onChange({target:{value:'## Edited unsaved text'}});});
    assert.equal(requests.length,0);
    assert.ok(JSON.stringify(view!.toJSON()).includes('Edited unsaved text'));
    const buttons=()=>root.findAllByType('button');
    await act(async()=>{buttons().find(b=>b.children.join('')==='Kiswahili (optional)')!.props.onClick();});
    assert.equal(root.findByType('textarea').props.value,'');assert.equal(requests.length,0);
    await act(async()=>{buttons().find(b=>b.children.join('')==='Save draft')!.props.onClick();});
    assert.equal(requests.length,1);assert.equal(requests[0].body.version,1);assert.ok(requests[0].url.endsWith(`/admin/articles/${article.id}`));
    const savedContent=requests[0].body.content as Record<string,unknown>;
    assert.deepEqual(Object.keys(savedContent).sort(),Object.keys(content).sort());
    assert.equal(savedContent.body_en,'## Edited unsaved text');
    assert.ok(!buttons().some(b=>b.children.join('')==='Submit for review'),'no review step in the Admin UI');
    await act(async()=>{buttons().find(b=>b.children.join('')==='Publish')!.props.onClick();});
    assert.ok(requests.length>=2);assert.ok(requests[1].url.endsWith('/submit'),'Publish starts the internal review steps');
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=windowBefore;}
});
