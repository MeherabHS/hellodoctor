/**
 * Search index for the People & Care directories (patients, doctors, Peer
 * Navigators). Each row's searchable fields are normalised once into a
 * single string when the list loads, so typing only scans prepared text:
 *
 * - case and accents are ignored ("Wanjiku" matches "wanjikú");
 * - every word of the query must match somewhere, in any order
 *   ("kamau nairobi" finds Neema Kamau in Nairobi);
 * - IDs match with or without punctuation ("PT-DA9A6", "#pt da9a6", "da9a6");
 * - phone numbers match by digits, with or without the country code
 *   ("0712 345 678" finds +254712345678).
 */
export type SearchIndex<T> = {row: T; text: string}[];

export function normalizeSearch(value: string): string {
  return value
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9@.+\s-]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Extra forms of one value so IDs and phone numbers match loosely. */
function variants(value: string): string[] {
  const base = normalizeSearch(value);
  const out = [base];
  const compact = base.replace(/[\s.+-]/g, '');
  if (compact && compact !== base) out.push(compact);
  const digits = value.replace(/\D/g, '');
  if (digits.length >= 7) {
    out.push(digits);
    // Kenyan numbers: +2547XXXXXXXX also matches 07XXXXXXXX.
    if (digits.startsWith('254') && digits.length >= 12) out.push(`0${digits.slice(3)}`);
  }
  return out;
}

export function buildSearchIndex<T>(rows: readonly T[], fields: (row: T) => (string | null | undefined)[]): SearchIndex<T> {
  return rows.map(row => ({
    row,
    text: ` ${fields(row).filter((f): f is string => Boolean(f && f.trim())).flatMap(variants).join(' ')} `,
  }));
}

/** Rows matching every word of the query, in their original order. */
export function searchIndex<T>(index: SearchIndex<T>, query: string): T[] {
  const words = normalizeSearch(query).split(' ').filter(Boolean)
    .map(w => w.replace(/^#/, ''))
    .flatMap(w => {
      const compact = w.replace(/[.+-]/g, '');
      const digits = w.replace(/\D/g, '');
      return [digits.length >= 7 && digits.length === compact.length ? (digits.startsWith('0') ? digits.slice(1) : digits) : compact || w];
    });
  if (!words.length) return index.map(e => e.row);
  return index.filter(e => words.every(w => e.text.includes(w))).map(e => e.row);
}
