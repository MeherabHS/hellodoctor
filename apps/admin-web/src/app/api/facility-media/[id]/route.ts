import {NextRequest,NextResponse} from 'next/server';
import {withAdminSession,backendUrl,backendHeaders} from '@/lib/session';

// Bounded binary transport for facility photos (same shape as article covers).
// GET previews the current photo; POST uploads; DELETE removes a draft's photo.
// Only image or JSON bodies are ever relayed; anything else is labelled as
// opaque bytes so a browser can never render it as a page.
function safeType(type:string|null){const base=(type||'').split(';')[0].trim().toLowerCase();return ['image/jpeg','image/png','application/json'].includes(base)?base:'application/octet-stream';}
async function forward(req:NextRequest,id:string){
  if(!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(id))return NextResponse.json({error:'Invalid facility ID.'},{status:400});
  const upload=req.method==='POST';const mime=req.headers.get('content-type')||'';
  let body:Uint8Array|undefined;
  if(upload){
    if(!['image/jpeg','image/png'].includes(mime))return NextResponse.json({error:'JPEG or PNG required.'},{status:415});
    const reader=req.body?.getReader();if(!reader)return NextResponse.json({error:'Image required.'},{status:400});
    const chunks:Uint8Array[]=[];let size=0;
    for(;;){const {done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>5*1024*1024){await reader.cancel();return NextResponse.json({error:'Photo is limited to 5 MB.'},{status:413});}chunks.push(value);}
    body=new Uint8Array(size);let offset=0;for(const chunk of chunks){body.set(chunk,offset);offset+=chunk.byteLength;}
  }
  const version=req.nextUrl.searchParams.get('version');
  const needsVersion=req.method==='POST'||req.method==='DELETE';
  if(needsVersion&&!/^\d{1,10}$/.test(version||''))return NextResponse.json({error:'Facility version required.'},{status:400});
  const target=`${backendUrl()}/api/v1/admin/facilities/${id}/image${needsVersion?`?version=${version}`:''}`;
  const send=async(access:string)=>fetch(target,{method:req.method,headers:{...(await backendHeaders()),Authorization:`Bearer ${access}`,'X-Client-Type':'ADMIN_WEB','X-Request-Id':crypto.randomUUID(),...(upload?{'Content-Type':mime}:{})},body:body as BodyInit|undefined,cache:'no-store',redirect:'error',signal:AbortSignal.timeout(15000)});
  try{
    const response=await withAdminSession(send);
    return new NextResponse(await response.arrayBuffer(),{status:response.status,headers:{'Content-Type':safeType(response.headers.get('Content-Type')),'Cache-Control':'private, no-store','X-Content-Type-Options':'nosniff'}});
  }catch{return NextResponse.json({error:'Facility photo service unavailable.'},{status:502});}
}
export async function GET(req:NextRequest,{params}:{params:Promise<{id:string}>}){return forward(req,(await params).id);}
export async function POST(req:NextRequest,{params}:{params:Promise<{id:string}>}){return forward(req,(await params).id);}
export async function DELETE(req:NextRequest,{params}:{params:Promise<{id:string}>}){return forward(req,(await params).id);}
