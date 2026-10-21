// Privacy boundary shared by browser diagnostics and its authenticated BFF.
// Mirrors apps/doctor-web/src/lib/client-incidents.ts exactly, adapted only
// for this app's client identifier and screen/operation vocabulary — both
// client-incidents.ts (client-side) and the backend's client_error_handlers.rs
// (server-side) must agree on these fixed lists, or every admin-web incident
// silently falls back to Application/unhandled_async.
// Never serialize an Error object, exception prose, request data or full URLs.
export type IncidentContext = { screen?: string; operation?: string; backendReference?: string };
const screens = ['Application', 'Auth', 'Patients', 'Settings', 'Notifications'];
const operations = ['render', 'unhandled_async', 'browser_error', 'load', 'login', 'verify_login', 'update', 'response_parse'];
const types = ['Exception', 'Error', 'TypeError', 'SyntaxError', 'RangeError', 'ReferenceError', 'API_FAILURE', 'RESPONSE_INVALID'];
export function sanitizedStack(stack: unknown): string {
  if (typeof stack !== 'string') return '';
  return (stack.slice(0, 12000).match(/\/_next\/static\/(?:chunks|webpack)\/[a-zA-Z0-9_./%\[\]-]+\.(?:js|tsx?):\d{1,7}:\d{1,7}/g) ?? []).slice(0, 24).join('\n').slice(0, 6000);
}
export function safeIncident(error: unknown, context: IncidentContext = {}, kind?: string) {
  const name = kind ?? (error instanceof Error ? error.name : 'Exception');
  const error_type = types.includes(name) ? name : 'Exception';
  return {
    client: 'ADMIN_WEB', error_type, message: `Unexpected client exception (${error_type}).`,
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
  return !(error instanceof DOMException && ['AbortError', 'TimeoutError'].includes(error.name));
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
      await send(safeIncident(error, context, kind));
      return true;
    } catch { return false; }
    finally { if (ownsSend) sending = false; }
  };
}
