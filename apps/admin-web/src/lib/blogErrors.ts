import { currentLocale, tr } from './i18n/core.ts';
import { apiErrorMessage } from './i18n/apiErrors.ts';

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
  if (/approval by another admin|approval required from another admin/i.test(message)) return tr('lib.independentApproval');
  if (message === 'Article media storage is not configured.') return tr('lib.coverUploadDeferred');
  if (status === 403) return tr('errors.FORBIDDEN');
  if (status === 401) return tr('errors.UNAUTHORIZED');
  if (status && status >= 500) return tr('errors.SERVICE_UNAVAILABLE');
  if (status && !message) return tr('lib.requestNotCompleted');
  if (error instanceof TypeError) return tr('errors.network');
  // Swahili: the error code's sentence plus the server's own detail.
  if (envelope && currentLocale() === 'sw') return apiErrorMessage(error, tr('lib.actionNotCompleted'));
  return message || tr('lib.actionNotCompleted');
}
