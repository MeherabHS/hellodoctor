import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const webCss = readFileSync(new URL('../src/app/web.css', import.meta.url), 'utf8');
const prototypeCss = readFileSync(new URL('../src/app/prototype.css', import.meta.url), 'utf8');
const commonTsx = readFileSync(new URL('../src/components/Common.tsx', import.meta.url), 'utf8');

function rule(css: string, selector: string): string {
  const start = css.indexOf(`${selector} {`);
  assert.notEqual(start, -1, `expected to find "${selector} {" in the stylesheet`);
  const end = css.indexOf('}', start);
  return css.slice(start, end + 1);
}

test('the loading spinner renders a circle from one self-contained rule, not a cross-file cascade', () => {
  const spinnerRule = rule(webCss, '.web-state-spinner');
  // The whole box model must live in this one rule: a non-replaced inline
  // element silently ignores width/height, so if `display` is ever split
  // into a different stylesheet from width/height again, load-order
  // fragility (Next.js does not guarantee stable cross-file global-CSS
  // order across every route) can silently re-squash this into a sliver.
  for (const required of ['display: inline-block', 'width: 24px', 'height: 24px', 'border-radius: 50%', 'animation: spinRing']) {
    assert.ok(spinnerRule.includes(required), `.web-state-spinner must declare "${required}" itself: ${spinnerRule}`);
  }
});

test('the spinner element only carries the one self-contained class, never the legacy split one', () => {
  assert.ok(commonTsx.includes('className="web-state-spinner"'), 'Common.tsx should render the spinner span with only web-state-spinner');
  assert.ok(!commonTsx.includes('loading-spinner-ring'), 'Common.tsx must not reintroduce the removed, conflicting loading-spinner-ring class');
});

test('prototype.css no longer defines a conflicting loading-spinner-ring box model', () => {
  assert.ok(!prototypeCss.includes('.loading-spinner-ring'), 'the old split/conflicting rule should be fully removed, not left dormant to be reintroduced later');
  // The shared @keyframes declaration is harmless to keep (keyframes do not
  // have cascade-order conflicts the way box-model properties do).
  assert.ok(prototypeCss.includes('@keyframes spinRing'));
});

test('reduced-motion preference still disables the spin animation', () => {
  assert.match(webCss, /@media \(prefers-reduced-motion: reduce\)\s*\{\s*\.web-state-spinner\s*\{\s*animation: none;/);
});
