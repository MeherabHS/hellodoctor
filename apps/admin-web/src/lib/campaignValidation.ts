import type { CampaignContent } from './api';

// Mirrors the backend's own limits exactly
// (`services/backend/src/api/v1/campaign_handlers.rs::CampaignContent::validate`)
// so the Admin sees a client-side error before ever submitting, not only
// after a 422 round trip.
export const CAMPAIGN_TITLE_MAX = 160;
export const CAMPAIGN_BODY_MAX = 600;

export function campaignFieldErrors(content: CampaignContent): string[] {
  const errors: string[] = [];
  for (const [label, value, max] of [
    ['English title', content.title_en, CAMPAIGN_TITLE_MAX],
    ['Kiswahili title', content.title_sw, CAMPAIGN_TITLE_MAX],
    ['English message', content.body_en, CAMPAIGN_BODY_MAX],
    ['Kiswahili message', content.body_sw, CAMPAIGN_BODY_MAX],
  ] as const) {
    const trimmed = value.trim();
    if (!trimmed) errors.push(`${label} is required.`);
    else if (trimmed.length > max) errors.push(`${label} must be ${max} characters or fewer.`);
  }
  return errors;
}
