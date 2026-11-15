/** Admin publishes directly (product-owner instruction 2026-11-14): there is
 *  no Submit for review, Approve or Return in the Admin UI. The backend still
 *  records its internal review steps for the audit trail, so publishing runs
 *  whichever of them the item still needs, from whatever state it is in. An
 *  item left part-way (for example by a network drop) is finished by the next
 *  Publish, never stranded in a review state the UI no longer offers. */
export type ReviewStep = 'submit' | 'approve';

export const UNPUBLISHED_STATUSES = ['DRAFT', 'PENDING_REVIEW', 'APPROVED'] as const;

export function isUnpublished(status: string): boolean {
  return (UNPUBLISHED_STATUSES as readonly string[]).includes(status);
}

/** Review steps still required before `status` can be published or scheduled. */
export function stepsBeforePublish(status: string): ReviewStep[] {
  if (status === 'DRAFT') return ['submit', 'approve'];
  if (status === 'PENDING_REVIEW') return ['approve'];
  return [];
}
