import type { CampaignContent } from './api';
import { tr } from './i18n/core.ts';

// Mirrors the backend's own limits exactly
// (`services/backend/src/api/v1/campaign_handlers.rs::CampaignContent::validate`)
// so the Admin sees a client-side error before ever submitting, not only
// after a 422 round trip.
export const CAMPAIGN_TITLE_MAX = 160;
export const CAMPAIGN_BODY_MAX = 600;

export function campaignFieldErrors(content: CampaignContent): string[] {
  const errors: string[] = [];
  for (const [label, value, max] of [
    [tr('lib.englishTitle'), content.title_en, CAMPAIGN_TITLE_MAX],
    [tr('lib.kiswahiliTitle'), content.title_sw, CAMPAIGN_TITLE_MAX],
    [tr('lib.englishMessage'), content.body_en, CAMPAIGN_BODY_MAX],
    [tr('lib.kiswahiliMessage'), content.body_sw, CAMPAIGN_BODY_MAX],
  ] as const) {
    const trimmed = value.trim();
    if (!trimmed) errors.push(tr('lib.fieldRequired', { label }));
    else if (trimmed.length > max) errors.push(tr('lib.fieldTooLong', { label, max }));
  }
  return errors;
}
