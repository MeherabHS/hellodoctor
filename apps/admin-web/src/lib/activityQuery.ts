/** Pure helpers for the Activity Log (no imports, unit-tested). */
export type ActivityArea = 'all' | 'content' | 'people' | 'operations' | 'campaigns' | 'finance' | 'security' | 'team';
export interface ActivityItem {
  id: string;
  at: string;
  actor_role: string;
  /** Only Admins are named; patients and doctors appear by role. */
  actor_email: string | null;
  is_you: boolean;
  action: string;
  resource_type: string;
  resource_id: string | null;
  result: string;
}
export interface ActivityPage { items: ActivityItem[]; next_before: string | null }
export interface ActivityFilter { area: ActivityArea; search: string; mine: boolean; before: string | null; limit?: number }

/** Search is limited server-side to letters, digits, spaces and _. */
export const SEARCH_PATTERN = /^[A-Za-z0-9_ ]{0,80}$/;

export function activityQuery(f: ActivityFilter): string {
  const q = new URLSearchParams();
  q.set('limit', String(f.limit ?? 50));
  if (f.area !== 'all') q.set('area', f.area);
  const search = f.search.trim();
  if (search && SEARCH_PATTERN.test(search)) q.set('search', search);
  if (f.mine) q.set('mine', 'true');
  if (f.before) q.set('before', f.before);
  return q.toString();
}

/** "ADMIN_ARTICLE_PUBLISHED" -> "Admin article published". */
export function humanize(code: string): string {
  const words = code.toLowerCase().split('_').filter(Boolean).join(' ');
  return words ? words[0].toUpperCase() + words.slice(1) : code;
}
