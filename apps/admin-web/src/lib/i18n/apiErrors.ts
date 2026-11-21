import { currentLocale, tr, type MessageKey } from './core.ts';

/** A backend error as the API helper throws it: `API Error [409]: {json}`. */
export interface ParsedApiError {
  status?: number;
  code?: string;
  message: string;
}

export function parseApiError(error: unknown): ParsedApiError {
  const original = error instanceof Error ? error.message : typeof error === 'string' ? error : '';
  const envelope = /^API Error \[(\d{3})\]:\s*([\s\S]*)$/.exec(original);
  if (!envelope) return { message: original };
  const status = Number(envelope[1]);
  try {
    const body = JSON.parse(envelope[2]);
    const detail = body?.error;
    if (typeof detail === 'string') return { status, message: detail };
    return {
      status,
      code: typeof detail?.code === 'string' ? detail.code : undefined,
      message: typeof detail?.message === 'string' ? detail.message : '',
    };
  } catch {
    return { status, message: '' };
  }
}

const CODE_KEYS: Record<string, MessageKey> = {
  VALIDATION_FAILED: 'errors.VALIDATION_FAILED',
  RESOURCE_NOT_FOUND: 'errors.RESOURCE_NOT_FOUND',
  RESOURCE_CONFLICT: 'errors.RESOURCE_CONFLICT',
  UNAUTHORIZED: 'errors.UNAUTHORIZED',
  FORBIDDEN: 'errors.FORBIDDEN',
  RATE_LIMIT_EXCEEDED: 'errors.RATE_LIMIT_EXCEEDED',
  PAYLOAD_TOO_LARGE: 'errors.PAYLOAD_TOO_LARGE',
  UNPROCESSABLE_ENTITY: 'errors.UNPROCESSABLE_ENTITY',
  PAYMENT_REQUIRED: 'errors.PAYMENT_REQUIRED',
  CAPACITY_LIMIT_REACHED: 'errors.CAPACITY_LIMIT_REACHED',
  SERVICE_UNAVAILABLE: 'errors.SERVICE_UNAVAILABLE',
  RATE_LIMIT_SERVICE_UNAVAILABLE: 'errors.SERVICE_UNAVAILABLE',
  INTERNAL_SERVER_ERROR: 'errors.INTERNAL_SERVER_ERROR',
  NOT_YET_OPEN: 'errors.NOT_YET_OPEN',
  WINDOW_CLOSED: 'errors.WINDOW_CLOSED',
};

function keyForStatus(status?: number): MessageKey | undefined {
  if (!status) return undefined;
  if (status === 401) return 'errors.UNAUTHORIZED';
  if (status === 403) return 'errors.FORBIDDEN';
  if (status === 404) return 'errors.RESOURCE_NOT_FOUND';
  if (status === 409) return 'errors.RESOURCE_CONFLICT';
  if (status === 413) return 'errors.PAYLOAD_TOO_LARGE';
  if (status === 429) return 'errors.RATE_LIMIT_EXCEEDED';
  if (status >= 500) return 'errors.SERVICE_UNAVAILABLE';
  if (status >= 400) return 'errors.VALIDATION_FAILED';
  return undefined;
}

/**
 * The text to show for a failed request. English: the server's own sentence
 * (never the raw JSON). Swahili: a translated sentence for the error code,
 * followed by the server's specific detail, so nothing is lost.
 */
export function apiErrorMessage(error: unknown, fallback: string): string {
  if (error instanceof TypeError) return tr('errors.network');
  const parsed = parseApiError(error);
  const key = (parsed.code && CODE_KEYS[parsed.code]) || keyForStatus(parsed.status);
  if (currentLocale() === 'sw') {
    if (!key) return parsed.message && !parsed.status ? parsed.message : fallback;
    const translated = tr(key);
    return parsed.message ? `${translated} (${parsed.message})` : translated;
  }
  if (parsed.message) return parsed.message;
  return key ? tr(key) : fallback;
}

/**
 * Sign-in screens get a plain `{error: string}` from the auth routes.
 * English keeps that sentence; Swahili shows a translated one by status
 * (with the route's detail in brackets, except for 401).
 */
export function authErrorMessage(status: number, serverText: unknown, fallback: string): string {
  const text = typeof serverText === 'string' && serverText ? serverText : '';
  if (currentLocale() !== 'sw') return text || fallback;
  if (status === 429) return tr('errors.RATE_LIMIT_EXCEEDED');
  if (status >= 500) return tr('errors.SERVICE_UNAVAILABLE');
  // 401 already has a specific translated sentence; other 4xx keep the detail.
  return status !== 401 && text ? `${fallback} (${text})` : fallback;
}
