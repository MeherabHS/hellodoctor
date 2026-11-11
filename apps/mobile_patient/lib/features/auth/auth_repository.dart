import '../../core/network/api_client.dart';
import '../../core/network/client_error_reporter.dart';
import 'auth_models.dart';

/// Talks to the real patient OTP contract confirmed by direct backend
/// inspection: `POST /auth/register-otp` then `POST /auth/verify-otp-and-login`
/// (NOT the Doctor side's email+password+MFA `/auth/doctor/*` routes — the
/// two roles use genuinely different endpoints and payload shapes, there is
/// no shared `/auth/{role}/...` parameterization today).
class AuthRepository {
  AuthRepository(this.api);
  final ApiClient api;

  dynamic _data(dynamic body) => (body as Map)['data'];

  Future<EmailChallengeResult> requestPatientLogin(
      {required String email, required String password}) async {
    final response = await api.post('/auth/patient/login',
        data: {'email': email, 'password': password});
    return EmailChallengeResult.fromJson(
        Map<String, dynamic>.from(_data(response.data) as Map));
  }

  Future<EmailChallengeResult> requestPatientRegistration(
      {required String email,
      required String phoneNumber,
      required String password,
      required String fullName,
      required String dateOfBirth,
      required String gender}) async {
    final response = await api.post('/auth/patient/register', data: {
      'email': email,
      'phone_number': phoneNumber,
      'password': password,
      'full_name': fullName,
      'date_of_birth': dateOfBirth,
      'gender': gender
    });
    return EmailChallengeResult.fromJson(
        Map<String, dynamic>.from(_data(response.data) as Map));
  }

  Future<PatientUser> verifyPatientEmailOtp(
      {required String challengeToken,
      required String otpCode,
      List<Map<String, String>> acceptedDocuments = const []}) async {
    final response = await api.post('/auth/patient/verify-otp', data: {
      'challenge_token': challengeToken,
      'otp_code': otpCode,
      if (acceptedDocuments.isNotEmpty) 'accepted_documents': acceptedDocuments,
    });
    final data = Map<String, dynamic>.from(_data(response.data) as Map);
    await api.saveTokens(
        data['access_token'] as String, data['refresh_token'] as String);
    return PatientUser.fromJson(Map<String, dynamic>.from(data['user'] as Map));
  }

  Future<OtpRequestResult> requestOtp(String phoneNumber) async {
    try {
      final response = await api
          .post('/auth/register-otp', data: {'phone_number': phoneNumber});
      return OtpRequestResult.fromJson(
          Map<String, dynamic>.from(_data(response.data) as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Auth', operation: 'request_otp');
      rethrow;
    }
  }

  /// Returns the signed-in [PatientUser] and persists the session tokens via
  /// [ApiClient.saveTokens] on success.
  Future<PatientUser> verifyOtp({
    required String phoneNumber,
    required String otpCode,
    List<Map<String, String>> acceptedDocuments = const [],
  }) async {
    try {
      final response = await api.post('/auth/verify-otp-and-login', data: {
        'phone_number': phoneNumber,
        'otp_code': otpCode,
        if (acceptedDocuments.isNotEmpty)
          'accepted_documents': acceptedDocuments,
      });
      final data = Map<String, dynamic>.from(_data(response.data) as Map);
      await api.saveTokens(
          data['access_token'] as String, data['refresh_token'] as String);
      return PatientUser.fromJson(
          Map<String, dynamic>.from(data['user'] as Map));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Auth', operation: 'verify_otp');
      rethrow;
    }
  }

  /// Step 1 of Phase 2 self-service password recovery. Always succeeds for
  /// a syntactically valid email, by backend design, regardless of whether
  /// an eligible account exists — see
  /// `password_reset_service.rs::request_reset`'s doc comment. Never throws
  /// for "no such account"; only network/validation/rate-limit failures
  /// surface as [ApiException].
  Future<void> requestPasswordReset({required String email}) async {
    await api.post('/auth/password-reset/request', data: {'email': email});
  }

  /// Step 2: returns the one-time [ResetAuthorizationResult.resetAuthorization]
  /// the final step requires. Unlike step 1, a wrong/expired code genuinely
  /// fails here (401) — by this point the caller has already proven receipt
  /// of the emailed code, so there is no enumeration concern left to protect.
  Future<ResetAuthorizationResult> verifyPasswordResetOtp({
    required String email,
    required String otpCode,
  }) async {
    final response = await api.post('/auth/password-reset/verify-otp',
        data: {'email': email, 'otp_code': otpCode});
    return ResetAuthorizationResult.fromJson(
        Map<String, dynamic>.from(_data(response.data) as Map));
  }

  /// Step 3: the only call that actually changes the password. Does not
  /// sign the user in — the backend revokes every existing session for the
  /// account as part of completing the reset, so the app must send the
  /// person back to an ordinary sign-in screen afterward, never auto-login.
  Future<void> completePasswordReset({
    required String resetAuthorization,
    required String newPassword,
  }) async {
    await api.post('/auth/password-reset/complete', data: {
      'reset_authorization': resetAuthorization,
      'new_password': newPassword,
    });
  }

  Future<void> logout() async {
    try {
      final refresh = await api.read('refresh_token');
      if (refresh != null) {
        await api.post('/auth/logout', data: {'refresh_token': refresh});
      }
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Auth', operation: 'unhandled_async');
      rethrow;
    }
  }
}
