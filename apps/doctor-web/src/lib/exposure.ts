// Next middleware does not expose a trustworthy socket peer. A TLS ingress must
// overwrite these headers and authenticate itself; never trust forwarded headers alone.
export type ExposureConfig = {
  environment?: string; origin?: string; proxySecret?: string; devOtp?: string;
};
export class ExposureError extends Error {
  readonly status: number;
  constructor(status: number) { super('Secure deployment configuration or ingress required.'); this.status = status; }
}
function sameSecret(a: string, b: string) {
  let difference = a.length ^ b.length;
  for (let i = 0; i < b.length; i++) difference |= (a.charCodeAt(i) || 0) ^ b.charCodeAt(i);
  return difference === 0;
}
function singleIp(value: string) {
  if (value.includes(':')) {
    try { return /^[0-9a-f:]+$/i.test(value) && new URL(`http://[${value}]`).hostname.length > 2; }
    catch { return false; }
  }
  const parts = value.split('.');
  return parts.length === 4 && parts.every(p => /^(0|[1-9][0-9]{0,2})$/.test(p) && Number(p) <= 255);
}
export function exposureHeaders(headers: Headers, config: ExposureConfig): Record<string, string> {
  if (!['development', 'test', 'staging', 'production'].includes(config.environment ?? '')) throw new ExposureError(503);
  if (config.environment === 'development' || config.environment === 'test') return {};
  let origin: URL;
  try { origin = new URL(config.origin ?? ''); } catch { throw new ExposureError(503); }
  const secret = config.proxySecret ?? '';
  if (origin.protocol !== 'https:' || origin.origin !== config.origin || secret.length < 32 ||
      /\s/.test(secret) || config.devOtp === 'true') throw new ExposureError(503);
  const client = headers.get('x-forwarded-for') ?? '';
  if (!sameSecret(headers.get('x-hellodoctor-proxy') ?? '', secret) ||
      headers.get('x-forwarded-proto') !== 'https' || !singleIp(client)) throw new ExposureError(403);
  return { 'X-Forwarded-Proto': 'https', 'X-Forwarded-For': client };
}
export function developmentOtpAllowed(config: ExposureConfig) {
  return config.environment === 'development' && config.devOtp === 'true';
}
