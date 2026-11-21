import { tr } from './i18n/core.ts';
// Facilities & Services: types and client-side validation mirroring the
// backend (services/backend/src/domain/facilities.rs). The server remains the
// authority; these checks give Admins immediate feedback.

export type PublicationStatus = 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';
export const weekdays = ['MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT', 'SUN'] as const;
export type Weekday = (typeof weekdays)[number];
export const weekdayLabels: Record<Weekday, string> = {
  MON: 'Monday', TUE: 'Tuesday', WED: 'Wednesday', THU: 'Thursday', FRI: 'Friday', SAT: 'Saturday', SUN: 'Sunday',
};

export interface Period { open: string; close: string; overnight?: boolean }
export interface DayHours { all_day?: boolean; periods?: Period[] }
export type OpeningHours = Partial<Record<Weekday, DayHours>>;

export interface FacilityContent {
  name: string;
  description: string;
  county: string;
  location_text: string;
  contact_phone: string;
  google_maps_url: string;
  services: string[];
  youth_friendly: boolean;
  /** null = opening hours not provided. */
  opening_hours: OpeningHours | null;
}

export interface ServiceCategory { code: string; name_en: string; name_sw: string; icon_key: string }

export interface AdminFacility extends FacilityContent {
  id: string;
  publication_status: PublicationStatus;
  version: number;
  has_image: boolean;
  image_version: string | null;
  published_at: string | null;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
  timezone: string;
  publish_problems: string[];
  status: { state: 'OPEN' | 'CLOSED' | 'OPEN_24_HOURS' | 'HOURS_NOT_AVAILABLE' };
}

export const blankFacility = (): FacilityContent => ({
  name: '', description: '', county: '', location_text: '', contact_phone: '', google_maps_url: '',
  services: [], youth_friendly: false, opening_hours: null,
});

export function editableFacility(f: FacilityContent): FacilityContent {
  return {
    name: f.name, description: f.description, county: f.county, location_text: f.location_text,
    contact_phone: f.contact_phone, google_maps_url: f.google_maps_url, services: [...f.services],
    youth_friendly: f.youth_friendly,
    opening_hours: f.opening_hours ? JSON.parse(JSON.stringify(f.opening_hours)) : null,
  };
}

/** Kenyan numbers as +254…, 254… or 0…; returns E.164 (+254 + 9 digits) or null. */
export function normalizeKenyanPhone(raw: string): string | null {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.length > 24 || !/^\+?[0-9 ().-]+$/.test(trimmed) || trimmed.indexOf('+') > 0) return null;
  const digits = trimmed.replace(/[^0-9]/g, '');
  const national = digits.startsWith('254') ? digits.slice(3) : digits.startsWith('0') ? digits.slice(1) : null;
  return national && /^[1-9][0-9]{8}$/.test(national) ? `+254${national}` : null;
}

/** Official Google Maps hosts/paths over HTTPS only (same rules as the server). */
export function isGoogleMapsLink(raw: string): boolean {
  const url = raw.trim();
  if (!url || url.length > 2048 || /[\s\u0000-\u001f]/.test(url)) return false;
  const match = /^https:\/\/([^/?#]+)([/?#].*)?$/.exec(url);
  if (!match) return false;
  const authority = match[1];
  if (/[@:\\%]/.test(authority)) return false;
  const host = authority.toLowerCase();
  const path = match[2] ?? '';
  if (host === 'maps.app.goo.gl') return path.length > 1 && path.startsWith('/');
  if (host === 'maps.google.com' || host === 'maps.google.co.ke') return true;
  if (['google.com', 'www.google.com', 'google.co.ke', 'www.google.co.ke'].includes(host)) {
    return path === '/maps' || path.startsWith('/maps/') || path.startsWith('/maps?');
  }
  return false;
}

const minutes = (hhmm: string, allow24: boolean): number | null => {
  const m = /^(\d{2}):(\d{2})$/.exec(hhmm);
  if (!m) return null;
  const h = Number(m[1]); const mi = Number(m[2]);
  if (mi > 59) return null;
  if (h <= 23) return h * 60 + mi;
  return allow24 && h === 24 && mi === 0 ? 1440 : null;
};

export function openingHoursErrors(hours: OpeningHours | null): string[] {
  if (!hours) return [];
  const errors: string[] = [];
  const all: [number, number][] = [];
  weekdays.forEach((day, index) => {
    const d = hours[day];
    if (!d) return;
    const label = weekdayLabels[day];
    const periods = d.periods ?? [];
    if (d.all_day && periods.length) { errors.push(`${label}: choose open 24 hours or periods, not both.`); return; }
    if (periods.length > 3) errors.push(`${label}: at most three opening periods.`);
    const spans: [number, number][] = [];
    if (d.all_day) spans.push([0, 1440]);
    for (const p of periods) {
      const open = minutes(p.open, false); const close = minutes(p.close, true);
      if (open === null || close === null) { errors.push(`${label}: times must be HH:MM.`); continue; }
      if (p.overnight) {
        if (close > open || close === 1440) errors.push(`${label}: an overnight period closes the next morning, at or before its opening time.`);
        spans.push([open, close + 1440]);
      } else {
        if (close <= open) errors.push(`${label}: closing time must be after opening time (tick "overnight" if it closes after midnight).`);
        spans.push([open, close]);
      }
    }
    spans.sort((a, b) => a[0] - b[0]);
    for (let i = 1; i < spans.length; i++) if (spans[i][0] < spans[i - 1][1]) errors.push(`${label}: opening periods overlap.`);
    all.push(...spans.map(([s, e]) => [index * 1440 + s, index * 1440 + e] as [number, number]));
  });
  all.sort((a, b) => a[0] - b[0]);
  for (let i = 1; i < all.length; i++) {
    if (all[i][0] < all[i - 1][1] && !errors.some(e => e.includes('overlap'))) errors.push(tr('lib.overnightOverlaps'));
  }
  const week = 7 * 1440;
  const last = all[all.length - 1]; const first = all[0];
  if (last && first && last[1] > week && first[0] < last[1] - week) errors.push("Sunday's overnight period overlaps Monday.");
  return [...new Set(errors)];
}

/** Field problems that block saving (a draft needs only a valid name). */
export function facilityErrors(c: FacilityContent): string[] {
  const errors: string[] = [];
  const name = c.name.trim();
  if (name.length < 2 || name.length > 120) errors.push(tr('lib.facilityNameLength'));
  if (c.description.trim().length > 2000) errors.push(tr('lib.descriptionLimit'));
  if (c.location_text.trim().length > 200) errors.push(tr('lib.locationLimit'));
  if (c.contact_phone.trim() && !normalizeKenyanPhone(c.contact_phone)) errors.push(tr('lib.validKenyanPhone'));
  if (c.google_maps_url.trim() && !isGoogleMapsLink(c.google_maps_url)) errors.push(tr('lib.directionsLink'));
  errors.push(...openingHoursErrors(c.opening_hours));
  return errors;
}

/** What still blocks publication. */
export function publishProblems(c: FacilityContent, hasImage: boolean): string[] {
  const p: string[] = [];
  if (!c.county) p.push(tr('lib.countyRequired'));
  if (!c.location_text.trim()) p.push(tr('lib.locationRequired'));
  if (!c.contact_phone.trim()) p.push(tr('lib.phoneRequired'));
  if (!c.google_maps_url.trim()) p.push(tr('lib.mapsRequired'));
  if (!c.services.length) p.push(tr('lib.selectService'));
  if (!hasImage) p.push('A facility photo is required.');
  return p;
}

/** "Copy Monday hours to weekdays" (Tuesday–Friday). */
export function copyMondayToWeekdays(hours: OpeningHours | null): OpeningHours {
  const next: OpeningHours = { ...(hours ?? {}) };
  const monday = next.MON ?? {};
  for (const day of ['TUE', 'WED', 'THU', 'FRI'] as Weekday[]) next[day] = JSON.parse(JSON.stringify(monday));
  return next;
}

export const statusLabel: Record<PublicationStatus, string> = { DRAFT: 'Draft', PUBLISHED: 'Published', ARCHIVED: 'Archived' };
