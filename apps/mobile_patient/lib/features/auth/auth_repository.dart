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
