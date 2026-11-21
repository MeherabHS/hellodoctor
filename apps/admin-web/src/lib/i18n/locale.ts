/**
 * Admin Web language (product owner, 2026-11-21): English or Swahili,
 * chosen with the EN | SW switch and remembered per browser in a cookie.
 * The cookie is read on the server too, so the first render is already in
 * the chosen language. The Incident log stays in English by instruction.
 */
export type Locale = 'en' | 'sw';

export const LOCALE_COOKIE = 'afya_admin_locale';

export function normalizeLocale(value?: string | null): Locale {
  return value === 'sw' ? 'sw' : 'en';
}

/** BCP 47 tag for dates and numbers. */
export function intlTag(locale: Locale): string {
  return locale === 'sw' ? 'sw-KE' : 'en-KE';
}

export function writeLocaleCookie(locale: Locale) {
  if (typeof document === 'undefined') return;
  const secure = typeof location !== 'undefined' && location.protocol === 'https:' ? '; secure' : '';
  document.cookie = `${LOCALE_COOKIE}=${locale}; path=/; max-age=31536000; samesite=lax${secure}`;
}
