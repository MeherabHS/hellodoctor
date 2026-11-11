// Doctor Web's Content-Security-Policy and Permissions-Policy, kept in one
// pure module so they can be tested. The Agora Web SDK needs outbound
// HTTPS/WSS to Agora's signalling gateways and stats collectors
// (`*.agora.io`, `*.sd-rtn.com`); with `connect-src 'self'` alone the SDK's
// join request is blocked by the browser and never settles. It also needs the
// microphone, so Permissions-Policy must allow it for this origin.
export const AGORA_CONNECT_SRC = ['https://*.agora.io', 'https://*.sd-rtn.com', 'wss://*.agora.io', 'wss://*.sd-rtn.com'];

export const PERMISSIONS_POLICY = 'camera=(self), microphone=(self), geolocation=(), payment=()';

export function buildCsp(nonce: string, dev: boolean): string {
  return ["default-src 'self'", `script-src 'self' 'nonce-${nonce}' https://challenges.cloudflare.com${dev ? " 'unsafe-eval'" : ''}`,
    "style-src 'self' 'unsafe-inline'", "img-src 'self' data: blob:", "font-src 'self'",
    `connect-src 'self' ${AGORA_CONNECT_SRC.join(' ')}${dev ? ' ws://127.0.0.1:3001' : ''}`, "object-src 'none'",
    "base-uri 'none'", "frame-src https://challenges.cloudflare.com", "frame-ancestors 'none'", "form-action 'self'"].join('; ');
}
