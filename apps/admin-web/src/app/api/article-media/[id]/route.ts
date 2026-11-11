import {NextRequest,NextResponse} from 'next/server';
import {withAdminSession,backendUrl,backendHeaders} from '@/lib/session';

// Dedicated bounded binary transport; the existing JSON proxy and campaigns are unchanged.
async function forward(req:NextRequest,id:string){
  if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id))return NextResponse.json({error:'Invalid article ID.'},{status:400});
  const upload=req.method==='POST';const mime=req.headers.get('content-type')||'';
  let body:Uint8Array|undefined;
  if(upload){
    if(!['image/jpeg','image/png'].includes(mime))return NextResponse.json({error:'JPEG or PNG required.'},{status:415});
    const reader=req.body?.getReader();if(!reader)return NextResponse.json({error:'Image required.'},{status:400});
    const chunks:Uint8Array[]=[];let size=0;
    for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>5*1024*1024){await reader.cancel();return NextResponse.json({error:'Cover is limited to 5 MB.'},{status:413});}chunks.push(value);}
    body=new Uint8Array(size);let offset=0;for(const chunk of chunks){body.set(chunk,offset);offset+=chunk.byteLength;}
  }
  const version=req.nextUrl.searchParams.get('version');
  if(upload&&!/^\d{1,10}$/.test(version||''))return NextResponse.json({error:'Article version required.'},{status:400});
  const target=`${backendUrl()}/api/v1/admin/articles/${id}/cover${upload?`?version=${version}`:''}`;
  const send=async(access:string)=>fetch(target,{method:req.method,headers:{...(await backendHeaders()),Authorization:`Bearer ${access}`,'X-Client-Type':'ADMIN_WEB','X-Request-Id':crypto.randomUUID(),...(upload?{'Content-Type':mime}:{})},body:body as BodyInit|undefined,cache:'no-store',redirect:'error',signal:AbortSignal.timeout(15000)});
  try{
    const response=await withAdminSession(send);
    return new NextResponse(await response.arrayBuffer(),{status:response.status,headers:{'Content-Type':response.headers.get('Content-Type')||'application/json','Cache-Control':'private, no-store','X-Content-Type-Options':'nosniff'}});
  }catch{return NextResponse.json({error:'Article media service unavailable.'},{status:502});}
}
export async function GET(req:NextRequest,{params}:{params:Promise<{id:string}>}){return forward(req,(await params).id);}
export async function POST(req:NextRequest,{params}:{params:Promise<{id:string}>}){return forward(req,(await params).id);}
