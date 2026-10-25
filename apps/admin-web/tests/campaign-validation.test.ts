import test from 'node:test';
import assert from 'node:assert/strict';
import { CAMPAIGN_BODY_MAX, CAMPAIGN_TITLE_MAX, campaignFieldErrors } from '../src/lib/campaignValidation.ts';
import type { CampaignContent } from '../src/lib/api.ts';

function content(overrides: Partial<CampaignContent> = {}): CampaignContent {
  return {
    title_en: 'New clinic hours',
    title_sw: 'Masaa mapya',
    body_en: 'Our clinic now opens earlier.',
    body_sw: 'Kliniki inafunguliwa mapema.',
    audience: 'PATIENTS',
    ...overrides,
  };
}

test('valid content has no field errors', () => {
  assert.deepEqual(campaignFieldErrors(content()), []);
});

test('empty or whitespace-only fields are rejected', () => {
  const errors = campaignFieldErrors(content({ title_en: '   ' }));
  assert.equal(errors.length, 1);
  assert.match(errors[0], /English title is required/);
});

test('fields over the backend limit are rejected, mirroring the server exactly', () => {
  const overLongTitle = 'x'.repeat(CAMPAIGN_TITLE_MAX + 1);
  const overLongBody = 'y'.repeat(CAMPAIGN_BODY_MAX + 1);
  const errors = campaignFieldErrors(content({ title_sw: overLongTitle, body_en: overLongBody }));
  assert.equal(errors.length, 2);
  assert.ok(errors.some((e) => /Kiswahili title must be 160/.test(e)));
  assert.ok(errors.some((e) => /English message must be 600/.test(e)));
});

test('a field at exactly the limit is accepted', () => {
  const exactTitle = 'x'.repeat(CAMPAIGN_TITLE_MAX);
  assert.deepEqual(campaignFieldErrors(content({ title_en: exactTitle })), []);
});

test('all four fields can fail independently in one pass', () => {
  const errors = campaignFieldErrors(content({ title_en: '', title_sw: '', body_en: '', body_sw: '' }));
  assert.equal(errors.length, 4);
});
