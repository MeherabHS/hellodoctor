// Server-only session helpers. Tokens live exclusively in HttpOnly cookies —
// they are set and read here, on the server, and never touch client-side
// JavaScript or localStorage. This is the one place that defines the cookie
// attributes; every route handler that needs a token goes through here
// rather than reading `cookies()` directly, so the security properties
// (HttpOnly, Secure in production, SameSite=strict, scoped to /api) stay
// consistent everywhere.
import { cookies, headers } from 'next/headers';
import { exposureHeaders } from './exposure';
import { coordinatedRefresh } from './refresh';

const ACCESS_COOKIE = 'hd_admin_access';
const REFRESH_COOKIE = 'hd_admin_refresh';
const SESSION_MARKER = 'hd_admin_session';

// Matches the backend's access-token lifetime (crypto.rs::ACCESS_TOKEN_TTL_SECONDS)
// so the cookie never outlives the token it holds.
const ACCESS_TOKEN_TTL_SECONDS = 15 * 60;
// Matches the backend's refresh-token lifetime (auth_service.rs::REFRESH_TOKEN_TTL_DAYS).
const REFRESH_TOKEN_TTL_SECONDS = 7 * 24 * 60 * 60;

const baseCookieOptions = {
  httpOnly: true,
  secure: process.env.NODE_ENV === 'production',
  sameSite: 'strict' as const,
  path: '/api',
};

export async function setSessionCookies(accessToken: string, refreshToken: string) {
  const store = await cookies();
  store.set(ACCESS_COOKIE, accessToken, { ...baseCookieOptions, maxAge: ACCESS_TOKEN_TTL_SECONDS });
  store.set(REFRESH_COOKIE, refreshToken, { ...baseCookieOptions, maxAge: REFRESH_TOKEN_TTL_SECONDS });
  // Page middleware cannot see /api-scoped tokens. This non-credential marker
  // gates only the shell; every data request still requires backend JWT auth.
  store.set(SESSION_MARKER, '1', { ...baseCookieOptions, path: '/', maxAge: REFRESH_TOKEN_TTL_SECONDS });
}

export async function clearSessionCookies() {
  const store = await cookies();
  const expired={...baseCookieOptions,expires:new Date(0),maxAge:0};
  store.set(ACCESS_COOKIE,'',expired);
  store.set(REFRESH_COOKIE,'',expired);
  store.set(SESSION_MARKER,'',{...expired,path:'/'});
}

export async function setSessionMarker(){
  (await cookies()).set(SESSION_MARKER,'1',{...baseCookieOptions,path:'/',maxAge:REFRESH_TOKEN_TTL_SECONDS});
}

export async function getAccessToken(): Promise<string | undefined> {
  const store = await cookies();
  return store.get(ACCESS_COOKIE)?.value;
}

export async function getRefreshToken(): Promise<string | undefined> {
  const store = await cookies();
  return store.get(REFRESH_COOKIE)?.value;
}

function sessionError(status:number,error:string,code?:string) {
  return Response.json({success:false,error,...(code?{code}:{})},{status,headers:{'Cache-Control':'private, no-store',...(code==='SESSION_REFRESH_IN_PROGRESS'?{'Retry-After':'1'}:{})}});
}

// Shared by JSON, private cover and clinical-grant transports. Only the Rust
// backend decides whether access/refresh credentials represent an active session.
export async function withAdminSession(send:(access:string)=>Promise<Response>):Promise<Response> {
  try {
    const access=await getAccessToken();
    let response=access?await send(access):null;
    if(response&&response.status!==401)return response;
    const refresh=await getRefreshToken();
    if(!refresh){await clearSessionCookies();return sessionError(401,'Not authenticated.');}
    const result=await coordinatedRefresh(refresh,async()=>{
      const res=await fetch(`${backendUrl()}/api/v1/auth/refresh`,{method:'POST',headers:{...(await backendHeaders()),'Content-Type':'application/json'},body:JSON.stringify({refresh_token:refresh}),cache:'no-store',redirect:'error',signal:AbortSignal.timeout(15000)});
      if(res.status===401||res.status===403)return null;
      if(!res.ok)throw new Error('Refresh service unavailable.');
      const payload=await res.json();const tokens=payload?.data;
      if(payload?.success!==true||typeof tokens?.access_token!=='string'||!tokens.access_token||typeof tokens?.refresh_token!=='string'||!tokens.refresh_token)throw new Error('Invalid refresh response.');
      return {access_token:tokens.access_token,refresh_token:tokens.refresh_token};
    });
    if(result.outcome==='raced')return sessionError(503,'Session renewal is in progress. Retry this request.','SESSION_REFRESH_IN_PROGRESS');
    if(result.outcome==='failed'){await clearSessionCookies();return sessionError(401,'Not authenticated.');}
    await setSessionCookies(result.tokens.access_token,result.tokens.refresh_token);
    response=await send(result.tokens.access_token);
    if(response.status===401)await clearSessionCookies();
    return response;
  } catch {
    // A timeout/dependency failure is not proof of revocation. Return no data,
    // retain cookies for recovery, and avoid turning temporary loss into logout.
    return sessionError(502,'Session or backend service temporarily unavailable.');
  }
}

export function backendUrl(): string {
  const url = process.env.BACKEND_URL;
  if (!url) {
    throw new Error('BACKEND_URL is not set (see .env.local.example).');
  }
  return url;
}

export async function backendHeaders(): Promise<Record<string, string>> {
  const incoming = await headers();
  return exposureHeaders(new Headers(incoming), { environment: process.env.ENVIRONMENT,
    origin: process.env.ADMIN_WEB_ORIGIN, proxySecret: process.env.WEB_PROXY_SECRET,
    devOtp: process.env.DEV_AUTH_OTP_ENABLED });
}
