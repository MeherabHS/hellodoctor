'use client';
import { createInflightReads } from './inflight.mjs';
import { useCallback, useEffect, useRef, useState } from 'react';
import { createIncidentReporter, operationContext, IncidentContext } from './client-incidents';
import { deliverCallTelemetry, CallEvent } from './call-telemetry';

let scope: string | null = null;
let generation = 0;
let incidentScreen = 'Application';
export function setIncidentScreen(screen:string) { incidentScreen = screen; }
export function runtimeGeneration() { return generation; }
const pending = new Set<AbortController>();
const inflightReads=createInflightReads();
export function clearRuntime() {
  generation++;
  inflightReads.clear();
  scope = null;
  pending.forEach(controller => controller.abort());
  pending.clear();
}
export class ApiError extends Error {
  constructor(public status: number, public code: string, message: string, public reference?: string) { super(message); }
}
const reportIncident = createIncidentReporter(() => scope !== null,
  async body => {
    const response = await fetch('/api/diagnostics', { method: 'POST', credentials: 'same-origin',
      headers: {'Content-Type':'application/json'}, body: JSON.stringify(body),
      signal: AbortSignal.timeout(3000), keepalive: true });
    if (!response.ok) throw new Error('Incident delivery failed');
  });
export function request<T>(path:string,options:RequestInit={}):Promise<T> {
 if((options.method??'GET').toUpperCase()==='GET'&&!options.signal&&!options.headers) {
  return inflightReads.run(`${generation}:${path}`,()=>performRequest<T>(path,options));
 }
 return performRequest<T>(path,options);
}
async function performRequest<T>(path: string, options: RequestInit = {}): Promise<T> {
  const version = generation;
  const controller = new AbortController();
  pending.add(controller);
  let responseReceived = false;
  try {
    const res = await fetch(path, { ...options, credentials: 'same-origin', cache: 'no-store', signal: options.signal ?? controller.signal, headers: { 'Content-Type': 'application/json', 'X-Request-Id':crypto.randomUUID(), 'X-Client-Type':'DOCTOR_WEB', ...options.headers } });
    responseReceived = true;
    if (version !== generation) throw new DOMException('Session changed', 'AbortError');
    const serverScope = res.headers.get('X-Doctor-Scope');
    if (serverScope && scope && serverScope !== scope) {
      window.dispatchEvent(new Event('doctor-session-expired'));
      throw new DOMException('Account changed', 'AbortError');
    }
    if (serverScope) scope = serverScope;
    let json;
    try { json = await res.json(); }
    catch (error) {
      void reportIncident(error, operationContext(path, options.method), 'RESPONSE_INVALID');
      throw new ApiError(502, 'RESPONSE_INVALID', 'The service returned an unexpected response.');
    }
    if (!res.ok) {
      if (res.status === 401 && scope && path.startsWith('/api/proxy/')) window.dispatchEvent(new Event('doctor-session-expired'));
      if (res.status >= 500) void reportIncident(new ApiError(res.status, 'API_FAILURE', ''),
        {...operationContext(path, options.method), backendReference:json.meta?.request_id}, 'API_FAILURE');
      throw new ApiError(res.status, json.error?.code ?? 'REQUEST_FAILED', json.error?.message ?? 'Request failed.', json.meta?.request_id);
    }
    if (!json || json.success !== true || !('data' in json)) {
      void reportIncident(new Error('Response shape invalid'), operationContext(path, options.method), 'RESPONSE_INVALID');
      throw new ApiError(502, 'RESPONSE_INVALID', 'The service returned an unexpected response.');
    }
    return json.data as T;
  } catch (error) {
    if (error instanceof ApiError || (error instanceof DOMException && error.name === 'AbortError')) throw error;
    if (responseReceived) {
      void reportIncident(error,operationContext(path, options.method),'RESPONSE_INVALID');
      throw new ApiError(502,'RESPONSE_INVALID','The service returned an unexpected response.');
    }
    throw new ApiError(0, 'NETWORK_UNAVAILABLE', 'Network unavailable. Check your connection and retry.');
  } finally { pending.delete(controller); }
}
export const api = <T,>(path: string, options?: RequestInit) => request<T>('/api/proxy/' + path, options);
export async function authorizedDocument(path: string): Promise<Blob> {
  const version = generation;
  const controller = new AbortController();
  pending.add(controller);
  let responseReceived = false;
  try {
    const response = await fetch(path, { credentials: 'same-origin', cache: 'no-store', signal: controller.signal });
    responseReceived = true;
    const serverScope = response.headers.get('X-Doctor-Scope');
    if (version !== generation || (serverScope && scope && serverScope !== scope)) throw new DOMException('Session changed', 'AbortError');
    if (!response.ok) {
      if (response.status === 401) window.dispatchEvent(new Event('doctor-session-expired'));
      const result = await response.json();
      if (response.status >= 500) void reportIncident(new ApiError(response.status, 'API_FAILURE', ''),
        {screen:'SmartRx',operation:'load_document',backendReference:result.meta?.request_id},'API_FAILURE');
      throw new ApiError(response.status, result.error?.code ?? 'DOCUMENT_UNAVAILABLE', result.error?.message ?? 'Document access is unavailable.', result.meta?.request_id);
    }
    const blob = await response.blob();
    if (version !== generation) throw new DOMException('Session changed', 'AbortError');
    return blob;
  } catch (error) {
    if (error instanceof ApiError || error instanceof DOMException) throw error;
    if (responseReceived) {
      void reportIncident(error,{screen:'SmartRx',operation:'load_document'},'RESPONSE_INVALID');
      throw new ApiError(502,'RESPONSE_INVALID','The document service returned an unexpected response.');
    }
    throw new ApiError(0,'NETWORK_UNAVAILABLE','Network unavailable. Check your connection and retry.');
  } finally { pending.delete(controller); }
}
export function post<T>(path: string, body: unknown, key?: string) {
  return api<T>(path, { method: 'POST', body: JSON.stringify(body), headers: key ? { 'Idempotency-Key': key } : undefined });
}
export function recordCallTelemetry(appointment:string,event:CallEvent,measurements?:{rtt_ms?:number;packet_loss_percent?:number}) {
  return deliverCallTelemetry(appointment,event,(path,body)=>post(path,body),measurements);
}
// XMLHttpRequest exposes actual browser-to-BFF byte progress. Credentials remain
// in HttpOnly cookies; access/refresh tokens never enter this client transport.
export function uploadPrescription<T>(appointmentId: string, body: unknown, key: string,
  onProgress: (fraction: number | null) => void): Promise<T> {
  const path = '/api/proxy/consultations/' + encodeURIComponent(appointmentId) + '/prescription';
  const version = generation;
  const controller = new AbortController();
  pending.add(controller);
  return new Promise<T>((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const abort = () => { xhr.abort(); reject(new DOMException('Session changed', 'AbortError')); };
    controller.signal.addEventListener('abort', abort, { once: true });
    xhr.upload.onprogress = event => {
      if (version === generation) onProgress(event.lengthComputable && event.total > 0 ? event.loaded / event.total : null);
    };
    xhr.upload.onload = () => { if (version === generation) onProgress(1); };
    xhr.onload = () => {
      try {
        if (version !== generation) throw new DOMException('Session changed', 'AbortError');
        const serverScope = xhr.getResponseHeader('X-Doctor-Scope');
        if (serverScope && scope && serverScope !== scope) {
          window.dispatchEvent(new Event('doctor-session-expired'));
          throw new DOMException('Account changed', 'AbortError');
        }
        if (serverScope) scope = serverScope;
        const json = JSON.parse(xhr.responseText);
        if (xhr.status < 200 || xhr.status >= 300) {
          if (xhr.status === 401 && scope) window.dispatchEvent(new Event('doctor-session-expired'));
          if (xhr.status >= 500) void reportIncident(new ApiError(xhr.status, 'API_FAILURE', ''),
            {...operationContext(path,'POST'),backendReference:json.meta?.request_id}, 'API_FAILURE');
          throw new ApiError(xhr.status, json.error?.code ?? 'REQUEST_FAILED', json.error?.message ?? 'Upload failed.', json.meta?.request_id);
        }
        resolve(json.data as T);
      } catch (error) {
        if (!(error instanceof ApiError) && !(error instanceof DOMException)) void reportIncident(error,operationContext(path,'POST'),'RESPONSE_INVALID');
        reject(error instanceof ApiError || error instanceof DOMException ? error : new ApiError(xhr.status, 'UPLOAD_RESPONSE_INVALID', 'Unable to confirm this upload. Please retry.'));
      }
    };
    xhr.onerror = () => reject(new ApiError(0, 'NETWORK_UNAVAILABLE', 'Network unavailable. Check your connection and retry.'));
    xhr.ontimeout = () => reject(new ApiError(0, 'UPLOAD_TIMEOUT', 'Upload timed out. Check your connection and retry.'));
    xhr.onabort = () => reject(new DOMException('Upload cancelled', 'AbortError'));
    xhr.onloadend = () => controller.signal.removeEventListener('abort', abort);
    try {
      xhr.open('POST', path);
      xhr.timeout = 120000;
      xhr.setRequestHeader('Content-Type', 'application/json');
      xhr.setRequestHeader('Idempotency-Key', key);
      xhr.setRequestHeader('X-Request-Id',crypto.randomUUID());
      xhr.send(JSON.stringify(body));
    } catch { reject(new ApiError(0, 'NETWORK_UNAVAILABLE', 'Unable to start this upload. Please retry.')); }
  }).finally(() => { pending.delete(controller); });
}
export function diagnostic(category: string, error?: unknown, context: IncidentContext = {}) {
  void reportIncident(error ?? new Error('Unexpected client exception'),
    {...context, screen:context.screen ?? incidentScreen, operation:context.operation ?? (category==='render-error'?'render':category==='browser-error'?'browser_error':'unhandled_async')});
}
export function errorText(error: unknown): string {
  if (!(error instanceof ApiError)) diagnostic('caught-unexpected',error);
  if (error instanceof ApiError) return error.message + (error.reference ? ` Reference: ${error.reference}` : '');
  return 'Unable to complete this request. Please retry.';
}
export function useResource<T>(path: string | null, initial?: T) {
  const [state, setState] = useState<{path:string|null;data?:T;loading:boolean;error:string}>({path,data:initial,loading:!!path,error:''});
  const counter = useRef(0);
  const refresh = useCallback(async () => {
    const version = ++counter.current;
    setState(previous=>({path,data:previous.path===path?previous.data:initial,loading:!!path,error:''}));
    if (!path) return;
    try {
      const result = await api<T>(path);
      if (version === counter.current) setState({path,data:result,loading:false,error:''});
    } catch (e) {
      if (version === counter.current) setState({path,loading:false,error:e instanceof DOMException && e.name === 'AbortError'?'':errorText(e)});
    }
  }, [path]);
  useEffect(() => { void refresh(); return () => { counter.current++; }; }, [refresh]);
  const current=state.path===path?state:{path,loading:!!path,error:'',data:undefined};
  const setData=useCallback((data:React.SetStateAction<T|undefined>)=>setState(previous=>previous.path===path?{...previous,data:typeof data==='function'?(data as (value:T|undefined)=>T|undefined)(previous.data):data}:previous),[path]);
  return { data:current.data, loading:current.loading, error:current.error, refresh, setData };
}
