export type ArticleStatus = 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';
export type SwahiliContent = { title: string; summary: string; body: string };
/** Optional topic and Swahili version (2026-11-14). Sending `extras` replaces both. */
export type ArticleExtras = { topic_key: string | null; swahili: SwahiliContent | null };
export type ArticleContent = { title: string; summary: string; body: string; references_list: string[]; extras?: ArticleExtras };
export const ARTICLE_TOPICS: [string, string][] = [['HIV_BASICS', 'HIV basics'], ['PREP', 'PrEP'], ['CONDOMS', 'Condoms'], ['SRH', 'Sexual & reproductive health'], ['STIGMA_DISCLOSURE', 'Stigma & disclosure'], ['HEALTHY_RELATIONSHIPS', 'Healthy relationships']];
export function topicLabel(key: string | null | undefined) { return ARTICLE_TOPICS.find(([k]) => k === key)?.[1] ?? null; }
export type DoctorArticle = ArticleContent & {
  id: string; author_id: string; author_name: string; specialty: string;
  status: ArticleStatus; version: number; published_at: string | null; updated_at: string;
  topic_key?: string | null; title_sw?: string | null; summary_sw?: string | null; body_sw?: string | null; cover_image_id?: string | null;
};
export function articleValidation(content: ArticleContent): string | undefined {
  for (const [name, text, maximum] of [['Title', content.title, 160], ['Summary', content.summary, 600], ['Article', content.body, 30000]] as const) {
    if (!text.trim() || [...text.trim()].length > maximum) return `${name} must contain 1–${maximum} characters.`;
  }
  if (content.references_list.length > 10 || content.references_list.some(r => !r.trim() || [...r].length > 500)) return 'Include at most 10 references of 1–500 characters.';
  const sw = content.extras?.swahili;
  if (sw) for (const [name, text, maximum] of [['Swahili title', sw.title, 160], ['Swahili summary', sw.summary, 600], ['Swahili article', sw.body, 30000]] as const) {
    if (!text.trim() || [...text.trim()].length > maximum) return `${name} must contain 1–${maximum} characters, or leave every Swahili field empty.`;
  }
}
export function articleActions(status: ArticleStatus) {
  return { editable: status !== 'ARCHIVED', publishable: status === 'DRAFT', archivable: status !== 'ARCHIVED' };
}
