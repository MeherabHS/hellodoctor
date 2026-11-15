class EmailChallengeResult {
  const EmailChallengeResult(
      {required this.challengeToken,
      required this.expiresInSeconds,
      this.debugOtpCode});
  final String challengeToken;
  final int expiresInSeconds;
  final String? debugOtpCode;
  factory EmailChallengeResult.fromJson(Map<String, dynamic> json) =>
      EmailChallengeResult(
        challengeToken: json['challenge_token']?.toString() ?? '',
        expiresInSeconds: (json['expires_in'] as num?)?.toInt() ?? 0,
        debugOtpCode: json['debug_otp_code']?.toString(),
      );
}

/// Response of `POST /auth/password-reset/verify-otp` (Phase 2: Secure
/// Email OTP Password Recovery). [resetAuthorization] is a single-use
/// bearer secret good only for the immediately following
/// `POST /auth/password-reset/complete` call — never logged, never put in
/// a URL, and discarded by the cubit the moment that call finishes (success
/// or failure).
class ResetAuthorizationResult {
  const ResetAuthorizationResult({
    required this.resetAuthorization,
    required this.expiresInSeconds,
  });
  final String resetAuthorization;
  final int expiresInSeconds;
  factory ResetAuthorizationResult.fromJson(Map<String, dynamic> json) =>
      ResetAuthorizationResult(
        resetAuthorization: json['reset_authorization']?.toString() ?? '',
        expiresInSeconds: (json['expires_in'] as num?)?.toInt() ?? 0,
      );
}

/// `user` object returned by `POST /auth/patient/verify-otp` — intentionally
/// minimal (`{id, role, phone_number}` only, confirmed by backend research;
/// the `POST /auth/refresh` response carries no `user` object at all, so a
/// silently-restored session has none of this beyond what's already in the
/// stored JWT claims). A patient's actual profile (name, DOB, gender, ...)
/// is a separate, currently-missing backend concern — see
/// `features/patient/profile/profile_repository.dart`.
class PatientUser {
  const PatientUser({
    required this.id,
    required this.role,
    required this.phoneNumber,
  });

  final String id;
  final String role;
  final String phoneNumber;

  factory PatientUser.fromJson(Map<String, dynamic> json) => PatientUser(
        id: json['id']?.toString() ?? '',
        role: json['role']?.toString() ?? 'PATIENT',
        phoneNumber: json['phone_number']?.toString() ?? '',
      );
}
