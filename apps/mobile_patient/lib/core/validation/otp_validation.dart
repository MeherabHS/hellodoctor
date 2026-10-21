/// Backend contract: `POST /auth/verify-otp-and-login` expects a 6-digit
/// numeric `otp_code` (see `services/backend/src/services/auth_service.rs`'s
/// `OTP_TTL_SECONDS`/`OTP_MAX_ATTEMPTS` constants and `AuthService`'s OTP
/// generation). This is a client-side pre-check only, saving a round trip for
/// an obviously malformed code — the backend remains the sole authority on
/// whether a code is actually correct, unexpired, and under the attempt cap.
final RegExp _sixDigitOtp = RegExp(r'^\d{6}$');

bool isValidOtpFormat(String input) => _sixDigitOtp.hasMatch(input.trim());
