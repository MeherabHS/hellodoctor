import {NextRequest,NextResponse} from 'next/server';
import {backendHeaders,backendUrl,setSessionMarker,withAdminSession} from '@/lib/session';

// Migration/return-visit recovery for browsers whose old short-lived shell marker
// expired. The response never includes credentials or upstream Admin data.
export async function GET(req:NextRequest){
  const response=await withAdminSession(async token=>fetch(`${backendUrl()}/api/v1/admin/overview`,{
    headers:{...(await backendHeaders()),Authorization:`Bearer ${token}`,'X-Client-Type':'ADMIN_WEB'},
    cache:'no-store',redirect:'error',signal:AbortSignal.timeout(15000),
  }));
  if(response.ok){
    await setSessionMarker();
    return NextResponse.redirect(new URL('/',process.env.ADMIN_WEB_ORIGIN||req.url),{headers:{'Cache-Control':'private, no-store'}});
  }
  if(response.status===401||response.status===403)return NextResponse.redirect(new URL('/login',process.env.ADMIN_WEB_ORIGIN||req.url),{headers:{'Cache-Control':'private, no-store'}});
  return response;
}
