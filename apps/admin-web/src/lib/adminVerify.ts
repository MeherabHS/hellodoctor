// Validates the body of POST /api/login/verify (Admin step 2). Admin, Doctor
// and Patient share one single-factor-OTP MFA shape — the emailed
// verification code is the only factor checked here. There is no
// authenticator-app code: the backend's AdminVerifyOtpRequest has no such
// field, so this never required (and must never again require) one.
export type AdminVerifyBody = { session_token: string; otp_code: string };
export type AdminVerifyValidationError = { error: string; status: 400 };

const MISSING_FIELDS_ERROR = 'session_token and otp_code are required.';

export function validateAdminVerifyBody(
  body: unknown,
): AdminVerifyBody | AdminVerifyValidationError {
  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    return { error: MISSING_FIELDS_ERROR, status: 400 };
  }
  const { session_token, otp_code } = body as Record<string, unknown>;
  if (
    typeof session_token !== 'string' ||
    typeof otp_code !== 'string' ||
    !session_token ||
    session_token.length > 256 ||
    !/^[0-9]{6}$/.test(otp_code)
  ) {
    return { error: MISSING_FIELDS_ERROR, status: 400 };
  }
  return { session_token, otp_code };
}
