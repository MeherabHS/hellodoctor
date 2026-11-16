import {NextRequest,NextResponse} from 'next/server';
import {getAccessToken,getRefreshToken} from '@/lib/session';

// The public YouTube thumbnail for a video id, fetched server-side so the
// Admin can use it as a video's cover (the browser's CSP allows only this
// origin). Signed-in Admins only; the id is the only input and is strictly
// validated, so this cannot be used to fetch anything else.
const MAX_BYTES=2*1024*1024;
export async function GET(_req:NextRequest,{params}:{params:Promise<{id:string}>}){
  const {id}=await params;
  if(!/^[A-Za-z0-9_-]{11}$/.test(id))return NextResponse.json({error:'Invalid video id.'},{status:400});
  if(!(await getAccessToken())&&!(await getRefreshToken()))return NextResponse.json({error:'Sign in required.'},{status:401});
  for(const size of ['maxresdefault','hqdefault']){
    try{
      const res=await fetch(`https://i.ytimg.com/vi/${id}/${size}.jpg`,{cache:'no-store',redirect:'error',signal:AbortSignal.timeout(8000)});
      if(!res.ok||!(res.headers.get('content-type')||'').startsWith('image/jpeg'))continue;
      const bytes=await res.arrayBuffer();
      if(!bytes.byteLength||bytes.byteLength>MAX_BYTES)continue;
      return new NextResponse(bytes,{status:200,headers:{'Content-Type':'image/jpeg','Cache-Control':'private, no-store','X-Content-Type-Options':'nosniff'}});
    }catch{/* try the next size */}
  }
  return NextResponse.json({error:'No YouTube cover found for this video.'},{status:404});
}
