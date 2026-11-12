// Validation of the four values handed to Agora's client.join(appId, channel,
// token, uid), mirroring the Web SDK v4.24.9's own local checks (token string
// 1..2047 ASCII, appId string 1..2047, channel `^[A-Za-z0-9 !#$%&()+\-:;<=.>?@[\]^_{}|~,]{1,64}$`,
// uid an integer 0..4294967295). Running it first turns an opaque SDK
// INVALID_PARAMS into a precise, sanitized diagnostic: only which field failed,
// its type and its length — never a value.
export interface JoinArgsReport { ok: boolean; field?: 'app_id' | 'channel_name' | 'token' | 'uid'; diagnostic: string }

const ASCII = /^[\x20-\x7E]+$/;
const CHANNEL = /^[a-zA-Z0-9 !#$%&()+\-:;<=.>?@[\]^_{}|~,]{1,64}$/;
const kind = (v: unknown) => (v === null ? 'null' : Array.isArray(v) ? 'array' : typeof v);
const len = (v: unknown) => (typeof v === 'string' ? String(v.length) : '-');

export function validateJoinArgs(session: unknown): JoinArgsReport {
  const s = (session && typeof session === 'object' ? session : {}) as Record<string, unknown>;
  const checks: Array<[JoinArgsReport['field'] & string, boolean]> = [
    ['app_id', typeof s.app_id === 'string' && s.app_id.length >= 1 && s.app_id.length <= 2047 && ASCII.test(s.app_id)],
    ['channel_name', typeof s.channel_name === 'string' && CHANNEL.test(s.channel_name)],
    ['token', typeof s.token === 'string' && s.token.length >= 1 && s.token.length <= 2047 && ASCII.test(s.token)],
    ['uid', typeof s.uid === 'number' && Number.isInteger(s.uid) && s.uid >= 0 && s.uid <= 4294967295],
  ];
  const first = checks.find(([, valid]) => !valid);
  const detail = checks.map(([f, valid]) => `${f}:${kind(s[f])}/${len(s[f])}/${valid ? 'ok' : 'bad'}`).join(' ');
  return { ok: !first, field: first?.[0], diagnostic: `join_args ${detail}` };
}
