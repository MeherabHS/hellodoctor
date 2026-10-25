// Realistic, platform-style truncation only — not an attempt to reproduce
// FCM/APNs pixel-exact wrapping, which varies by device/OS version. These
// thresholds are deliberately conservative (shorter than most real devices
// allow) so an Admin who clears this preview's truncation warning is safe on
// real devices too.
export const PREVIEW_TITLE_LIMIT = 65;
export const PREVIEW_BODY_LIMIT = 120;

export interface TruncatedText {
  shown: string;
  truncated: boolean;
}

export function truncateForPreview(text: string, limit: number): TruncatedText {
  if (text.length <= limit) return { shown: text, truncated: false };
  return { shown: `${text.slice(0, limit - 1).trimEnd()}…`, truncated: true };
}
