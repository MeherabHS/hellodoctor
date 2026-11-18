// Unread state for the Admin notification bell. A notice counts as new until
// either the bell is opened (which advances the `seen` timestamp to the newest
// notice) or that notice is opened on its own from its pop-up (`viewed`).
export const MAX_VIEWED = 200;

export function isNoticeNew(at: string, id: string, seen: number, viewed: readonly string[]): boolean {
  return new Date(at).getTime() > seen && !viewed.includes(id);
}

/** Records one notice as viewed, without duplicates, keeping the newest MAX_VIEWED. */
export function withViewed(viewed: readonly string[], id: string): string[] {
  if (viewed.includes(id)) return [...viewed];
  return [...viewed, id].slice(-MAX_VIEWED);
}
