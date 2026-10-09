export type ArticleStatus = 'DRAFT' | 'PUBLISHED' | 'ARCHIVED';
export type ArticleContent = { title: string; summary: string; body: string; references_list: string[] };
export type DoctorArticle = ArticleContent & {
  id: string; author_id: string; author_name: string; specialty: string;
  status: ArticleStatus; version: number; published_at: string | null; updated_at: string;
};
export function articleValidation(content: ArticleContent): string | undefined {
  for (const [name, text, maximum] of [['Title', content.title, 160], ['Summary', content.summary, 600], ['Article', content.body, 30000]] as const) {
    if (!text.trim() || [...text.trim()].length > maximum) return `${name} must contain 1–${maximum} characters.`;
  }
  if (content.references_list.length > 10 || content.references_list.some(r => !r.trim() || [...r].length > 500)) return 'Include at most 10 references of 1–500 characters.';
}
export function articleActions(status: ArticleStatus) {
  return { editable: status !== 'ARCHIVED', publishable: status === 'DRAFT', archivable: status !== 'ARCHIVED' };
}
