export const independentApprovalMessage = 'Approval required from another Admin.';

/** Translate the existing API helper's errors only for CMS screens. Authorization stays server-side. */
export function blogErrorMessage(error: unknown): string {
  const original = error instanceof Error ? error.message : '';
  const envelope = /^API Error \[(\d{3})\]:\s*([\s\S]*)$/.exec(original);
  let status: number | undefined;
  let message = original;
  if (envelope) {
    status = Number(envelope[1]);
    try {
      const body = JSON.parse(envelope[2]);
      const detail = typeof body?.error === 'string' ? body.error : body?.error?.message;
      message = typeof detail === 'string' ? detail : '';
    } catch {
      message = '';
    }
  }
  if (/approval by another admin|approval required from another admin/i.test(message)) return independentApprovalMessage;
  if (message === 'Article media storage is not configured.') {
    return 'Cover upload deferred — awaiting Azure Blob integration. No cover was uploaded.';
  }
  if (status === 403) return 'You do not have permission to perform this action.';
  if (status === 401) return 'Your session expired. Please sign in again.';
  if (status && status >= 500) return 'The server is temporarily unavailable. Please retry.';
  if (status && !message) return 'The request could not be completed. Please retry.';
  if (error instanceof TypeError) return 'Unable to reach the server. Check your connection and retry.';
  return message || 'The action could not be completed. Please retry.';
}
