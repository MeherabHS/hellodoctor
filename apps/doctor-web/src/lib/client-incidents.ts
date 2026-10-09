// Privacy boundary shared by browser diagnostics and its authenticated BFF.
// Never serialize an Error object, exception prose, request data or full URLs.
export type IncidentContext = { screen?: string; operation?: string; backendReference?: string };
const screens = ['Application','Auth','Queue','Chat','SmartRx','Schedule','Wallet','Qna','Telehealth','Settings','Notifications','Patients'];
const operations = ['render','unhandled_async','browser_error','load','send_message','create_slot','block_slot','cancel_slot','book_appointment','login','verify_login','load_consultation','upload_prescription','complete_consultation','call_telemetry','mint_rtc_token','load_document','update','response_parse'];
const types = ['Exception','Error','TypeError','SyntaxError','RangeError','ReferenceError','API_FAILURE','RESPONSE_INVALID'];
export function sanitizedStack(stack: unknown): string {
  if (typeof stack !== 'string') return '';
  return (stack.slice(0,12000).match(/\/_next\/static\/(?:chunks|webpack)\/[a-zA-Z0-9_./%\[\]-]+\.(?:js|tsx?):\d{1,7}:\d{1,7}/g) ?? []).slice(0,24).join('\n').slice(0,6000);
}
export function safeIncident(error: unknown, context: IncidentContext = {}, kind?: string) {
  const name = kind ?? (error instanceof Error ? error.name : 'Exception');
  const error_type = types.includes(name) ? name : 'Exception';
  return {
    client: 'DOCTOR_WEB', error_type, message: `Unexpected client exception (${error_type}).`,
    stack_trace: sanitizedStack(error instanceof Error ? error.stack : ''),
    screen: screens.includes(context.screen ?? '') ? context.screen! : 'Application',
    operation: operations.includes(context.operation ?? '') ? context.operation! : 'unhandled_async',
    platform: 'web', occurred_at: new Date().toISOString(),
    backend_reference: /^[a-f0-9-]{36}$/i.test(context.backendReference ?? '') ? context.backendReference : undefined,
  };
}
export function unexpected(error: unknown): boolean {
  if (error && typeof error === 'object' && 'status' in error && typeof error.status === 'number') {
    return error.status >= 500;
  }
  return !(error instanceof DOMException && ['AbortError','TimeoutError'].includes(error.name));
}
export function operationContext(path: string, method = 'GET'): IncidentContext {
  // Never retain IDs/query strings. Only fixed route vocabulary becomes context.
  if (path.includes('/auth/')) return {screen:'Auth',operation:path.endsWith('verify')?'verify_login':'login'};
  if (path.includes('/telemetry')) return {screen:'Telehealth',operation:'call_telemetry'};
  if (path.includes('/rtc-token')) return {screen:'Telehealth',operation:'mint_rtc_token'};
  if (/\/chat\/[^/]+\/messages/.test(path)) return {screen:'Chat',operation:method==='POST'?'send_message':'load'};
  if (path.includes('slots')) return {screen:'Schedule',operation:method==='POST'?'create_slot':method==='PATCH'?'block_slot':method==='DELETE'?'cancel_slot':'load'};
  if (path.includes('/book')) return {screen:'Schedule',operation:'book_appointment'};
  if (path.includes('prescription')) return {screen:'SmartRx',operation:'upload_prescription'};
  if (path.includes('/complete')) return {screen:'SmartRx',operation:'complete_consultation'};
  if (path.includes('document')) return {screen:'SmartRx',operation:'load_document'};
  if (path.includes('wallet')) return {screen:'Wallet',operation:'load'};
  if (path.includes('qna')) return {screen:'Qna',operation:method==='POST'?'update':'load'};
  if (path.includes('notification')) return {screen:'Notifications',operation:'load'};
  return {screen:'Application',operation:method==='GET'?'load':'update'};
}
export function createIncidentReporter(authenticated: () => boolean,
  send: (body: ReturnType<typeof safeIncident>) => Promise<unknown>) {
  let sending = false;
  const seen = new WeakSet<object>();
  return async (error: unknown, context: IncidentContext = {}, kind?: string): Promise<boolean> => {
    let ownsSend = false;
    try {
      if (sending || !authenticated() || !unexpected(error)) return false;
      if (error && typeof error === 'object') {
        if (seen.has(error)) return false;
        seen.add(error);
      }
      sending = true; ownsSend = true;
      await send(safeIncident(error,context,kind));
      return true;
    } catch { return false; }
    finally { if (ownsSend) sending = false; }
  };
}
