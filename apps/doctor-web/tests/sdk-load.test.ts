import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';

// Regression for the production failure `[doctor-web:call] stage=sdk_load name=TypeError`.
// Root cause: ConsoleGuard froze Object.prototype on mount, which made `toString` read-only;
// the Agora Web SDK (strict mode) assigns `toString` on its own objects while its module
// evaluates, so `import('agora-rtc-sdk-ng')` threw
// "Cannot assign to read only property 'toString'".
const READ_ONLY = /read only property/;

function sources(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) sources(p, out);
    else if (/\.(ts|tsx|js|mjs)$/.test(name)) out.push(p);
  }
  return out;
}

test('no Doctor Web source freezes or seals the built-in prototypes', () => {
  const offenders = sources('src').filter(f =>
    /Object\.(freeze|seal|preventExtensions)\(\s*(Object|Function|Array|String)\.prototype/.test(readFileSync(f, 'utf8')));
  assert.deepEqual(offenders, []);
});

function loadSdk(freezeObjectPrototype: boolean): Error | null {
  const require = createRequire(import.meta.url);
  const code = readFileSync(require.resolve('agora-rtc-sdk-ng'), 'utf8');
  const noop = () => undefined;
  const ctx: Record<string, unknown> = { console: { log: noop, warn: noop, error: noop, info: noop, debug: noop }, URL, TextEncoder, TextDecoder,
    setTimeout: noop, clearTimeout: noop, setInterval: noop, clearInterval: noop, addEventListener: noop, removeEventListener: noop,
    location: { href: 'https://doctor.test/', protocol: 'https:', hostname: 'doctor.test', origin: 'https://doctor.test' },
    navigator: { userAgent: 'Mozilla/5.0 Chrome/120', platform: 'Win32', mediaDevices: {} }, document: { createElement: () => ({}), addEventListener: noop } };
  ctx.self = ctx; ctx.window = ctx;
  vm.createContext(ctx);
  if (freezeObjectPrototype) vm.runInContext('Object.freeze(Object.prototype)', ctx);
  try { vm.runInContext(code, ctx, { timeout: 15000 }); return null; } catch (e) { return e as Error; }
}

test('the real Agora SDK cannot evaluate with a frozen Object.prototype (documents the root cause)', () => {
  const err = loadSdk(true);
  assert.ok(err, 'expected the SDK to throw');
  assert.match(err!.message, READ_ONLY);
});

test('the real Agora SDK does not hit the read-only error with an untouched Object.prototype', () => {
  const err = loadSdk(false);
  // This headless context has no real DOM, so some other stub error is acceptable;
  // the read-only `toString` failure must not occur.
  if (err) assert.doesNotMatch(err.message, READ_ONLY);
});
