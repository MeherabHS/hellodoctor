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

test('sensitive author sees a disabled approval control and stays pending without transport',async()=>{
  const previous=globalThis.window;globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window & typeof globalThis;
  const fetchBefore=globalThis.fetch;let requests=0;globalThis.fetch=async()=>{requests++;throw new Error('Self approval must not send a request');};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={{...article,status:'PENDING_REVIEW',can_approve:false,approval_block_reason:'INDEPENDENT_REVIEW_REQUIRED'}} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    const approve=view!.root.findAllByType('button').find(b=>b.children.join('')==='Approve')!;
    assert.equal(approve.props.disabled,true);
    assert.ok(JSON.stringify(view!.toJSON()).includes('Approval required from another Admin.'));
    await act(async()=>{approve.props.onClick();});
    assert.equal(requests,0);assert.ok(JSON.stringify(view!.toJSON()).includes('PENDING REVIEW'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('authorized independent reviewer can approve through the existing endpoint',async()=>{
  const previous=globalThis.window;globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window & typeof globalThis;
  const fetchBefore=globalThis.fetch;let requestPath='';
  globalThis.fetch=async(url)=>{requestPath=String(url);return new Response(JSON.stringify({success:true,data:{...article,status:'APPROVED',version:2,approved_version:1}}));};
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={{...article,status:'PENDING_REVIEW',can_approve:true,approval_block_reason:null}} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    const approve=view!.root.findAllByType('button').find(b=>b.children.join('')==='Approve')!;assert.equal(approve.props.disabled,false);
    await act(async()=>{approve.props.onClick();});assert.ok(requestPath.endsWith('/approve'));assert.ok(JSON.stringify(view!.toJSON()).includes('APPROVED'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('server rejection remains readable and preserves pending review when permission changes',async()=>{
  const previous=globalThis.window;globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window & typeof globalThis;
  const fetchBefore=globalThis.fetch;
  globalThis.fetch=async()=>new Response(JSON.stringify({success:false,error:{code:'FORBIDDEN',message:'Sensitive health articles require approval by another Admin. This article remains pending review.'}}),{status:403});
  let view:ReactTestRenderer|undefined;
  try{
    await act(async()=>{view=create(<ArticleEditor initial={{...article,status:'PENDING_REVIEW',can_approve:true,approval_block_reason:null}} categories={[category]} onSaved={()=>{}} onClose={()=>{}}/>);});
    await act(async()=>{view!.root.findAllByType('button').find(b=>b.children.join('')==='Approve')!.props.onClick();});
    const output=JSON.stringify(view!.toJSON());assert.ok(output.includes('Approval required from another Admin.'));assert.ok(output.includes('PENDING REVIEW'));assert.ok(!output.includes('API Error'));assert.ok(!output.includes('FORBIDDEN'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=previous;}
});

test('failed cover upload preserves the saved draft and selection without reporting success',async()=>{
  const windowBefore=globalThis.window;const fetchBefore=globalThis.fetch;const readerBefore=globalThis.FileReader;
  globalThis.window={addEventListener(){},removeEventListener(){}} as unknown as Window & typeof globalThis;
  globalThis.FileReader=class{result='data:image/png;base64,fixture';onload:(()=>void)|null=null;readAsDataURL(){this.onload?.();}abort(){}} as unknown as typeof FileReader;
  const requests:string[]=[];let refreshed=0;let view:ReactTestRenderer|undefined;
  globalThis.fetch=async(url)=>{requests.push(String(url));return requests.length===1
    ?new Response(JSON.stringify({success:true,data:{...article,version:2}}))
    :new Response(JSON.stringify({success:false,error:{message:'Article media storage is not configured.'}}),{status:409});};
  try{
    await act(async()=>{view=create(<ArticleEditor initial={article} categories={[category]} onSaved={()=>{refreshed++;}} onClose={()=>{}}/>);});
    await act(async()=>{view!.root.findAllByType('input').find(input=>input.props.type==='file')!.props.onChange({target:{files:[new File(['fixture'],'cover.png',{type:'image/png'})]}});});
    await act(async()=>{view!.root.findAllByType('button').find(button=>button.children.join('')==='Save draft')!.props.onClick();});
    const output=JSON.stringify(view!.toJSON());
    assert.equal(requests.length,2);assert.ok(requests[1].endsWith(`/${article.id}?version=2`));assert.equal(refreshed,1);
    assert.ok(output.includes('Text draft saved.'));assert.ok(output.includes('awaiting Azure Blob integration'));assert.ok(output.includes('No cover was uploaded.'));
    assert.ok(view!.root.findAllByType('button').some(button=>button.children.join('')==='Remove selected cover'));
    assert.equal(view!.root.findAll(node=>node.props.className==='cms-notice').length,0);
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
test('editor typing and language switching remain local; draft save and review are explicit',async()=>{
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
    await act(async()=>{buttons().find(b=>b.children.join('')==='Kiswahili')!.props.onClick();});
    assert.equal(root.findByType('textarea').props.value,'');assert.equal(requests.length,0);
    await act(async()=>{buttons().find(b=>b.children.join('')==='Save draft')!.props.onClick();});
    assert.equal(requests.length,1);assert.equal(requests[0].body.version,1);assert.ok(requests[0].url.endsWith(`/admin/articles/${article.id}`));
    const savedContent=requests[0].body.content as Record<string,unknown>;
    assert.deepEqual(Object.keys(savedContent).sort(),Object.keys(content).sort());
    assert.equal(savedContent.body_en,'## Edited unsaved text');
    await act(async()=>{buttons().find(b=>b.children.join('')==='Submit for review')!.props.onClick();});
    assert.equal(requests.length,2);assert.ok(requests[1].url.endsWith('/submit'));
  }finally{if(view)act(()=>view!.unmount());globalThis.fetch=fetchBefore;globalThis.window=windowBefore;}
});
