import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_models.dart';
import 'package:hellodoctor_mobile_patient/features/auth/auth_repository.dart';

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `apps/mobile/test`'s equivalent harnesses).
class FakeAuthRepository extends AuthRepository {
  FakeAuthRepository(super.api);

  OtpRequestResult? otpResult;
  Object? otpError;
  PatientUser? verifyResult;
  Object? verifyError;
  String? lastRequestedPhone;
  ({String phone, String code})? lastVerified;

  Object? resetRequestError;
  String? lastResetRequestedEmail;
  ResetAuthorizationResult? resetVerifyResult;
  Object? resetVerifyError;
  ({String email, String code})? lastResetVerified;
  Object? resetCompleteError;
  ({String authorization, String password})? lastResetCompleted;

  @override
  Future<void> requestPasswordReset({required String email}) async {
    lastResetRequestedEmail = email;
    if (resetRequestError != null) throw resetRequestError!;
  }

  @override
  Future<ResetAuthorizationResult> verifyPasswordResetOtp({
    required String email,
    required String otpCode,
  }) async {
    lastResetVerified = (email: email, code: otpCode);
    if (resetVerifyError != null) throw resetVerifyError!;
    return resetVerifyResult!;
  }

  @override
  Future<void> completePasswordReset({
    required String resetAuthorization,
    required String newPassword,
  }) async {
    lastResetCompleted =
        (authorization: resetAuthorization, password: newPassword);
    if (resetCompleteError != null) throw resetCompleteError!;
  }

  @override
  Future<OtpRequestResult> requestOtp(String phoneNumber) async {
    lastRequestedPhone = phoneNumber;
    if (otpError != null) throw otpError!;
    return otpResult!;
  }

  @override
  Future<PatientUser> verifyOtp(
      {required String phoneNumber,
      required String otpCode,
      List<Map<String, String>> acceptedDocuments = const []}) async {
    lastVerified = (phone: phoneNumber, code: otpCode);
    if (verifyError != null) throw verifyError!;
    await api.saveTokens('access-token', 'refresh-token');
    return verifyResult!;
  }
}

class MemoryStore implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

void main() {
  late ApiClient api;
  late FakeAuthRepository repo;
  late AuthCubit cubit;

  setUp(() {
    // An explicit in-memory TokenStore — the default `SecureTokenStore`
    // touches a real platform channel that isn't available under `flutter
    // test`, which would otherwise make `saveTokens` fail silently (caught
    // by the cubit's own catch-all) rather than actually exercising the
    // SignedIn path this test is for.
    api = ApiClient(dio: Dio(), tokenStore: MemoryStore());
    repo = FakeAuthRepository(api);
    cubit = AuthCubit(api, repository: repo);
  });

  tearDown(() => cubit.close());

  group('requestOtp', () {
    test('rejects a structurally invalid phone number without any network call',
        () async {
      await cubit.requestOtp('12345');
      expect(cubit.state, isA<AuthFailure>());
      expect(repo.lastRequestedPhone, isNull);
    });

    test('normalizes a valid local number before calling the repository',
        () async {
      repo.otpResult = const OtpRequestResult(
          sessionToken: 's', expiresInSeconds: 120, debugOtpCode: '584920');
      await cubit.requestOtp('0712345678');
      expect(repo.lastRequestedPhone, '+254712345678');
      expect(cubit.state, isA<OtpSent>());
      final state = cubit.state as OtpSent;
      expect(state.phoneNumber, '+254712345678');
      expect(state.expiresInSeconds, 120);
      // A backend debug code is never surfaced unless this client was built
      // with both explicit development compile-time flags.
      expect(state.debugOtpCode, isNull);
    });

    test('surfaces a repository failure as AuthFailure', () async {
      repo.otpError = const ApiException(429, 'Too many attempts.');
      await cubit.requestOtp('0712345678');
      expect(cubit.state, isA<AuthFailure>());
    });
  });

  group('verifyOtp', () {
    test('does nothing if no phone number was requested first', () async {
      await cubit.verifyOtp('123456');
      expect(cubit.state, isA<SignedOut>());
      expect(repo.lastVerified, isNull);
    });

    test('rejects a malformed code without calling the repository', () async {
      repo.otpResult =
          const OtpRequestResult(sessionToken: 's', expiresInSeconds: 120);
      await cubit.requestOtp('0712345678');
      await cubit.verifyOtp('abc');
      expect(cubit.state, isA<AuthFailure>());
      expect(repo.lastVerified, isNull);
    });

    test('transitions to SignedIn on a valid code, not using session_token',
        () async {
      repo.otpResult = const OtpRequestResult(
          sessionToken: 'ignored-by-step-2', expiresInSeconds: 120);
      repo.verifyResult = const PatientUser(
          id: 'p1', role: 'PATIENT', phoneNumber: '+254712345678');
      await cubit.requestOtp('0712345678');
      await cubit.verifyOtp('654321');
      expect(repo.lastVerified?.phone, '+254712345678');
      expect(repo.lastVerified?.code, '654321');
      expect(cubit.state, isA<SignedIn>());
    });

    test('an invalid/expired code surfaces a specific message', () async {
      repo.otpResult =
          const OtpRequestResult(sessionToken: 's', expiresInSeconds: 120);
      repo.verifyError = const ApiException(401, 'Invalid OTP.');
      await cubit.requestOtp('0712345678');
      await cubit.verifyOtp('000000');
      expect(cubit.state, isA<AuthFailure>());
      expect((cubit.state as AuthFailure).message, contains('expired'));
    });
  });

  group('Phase 2: password reset', () {
    test(
        'requestPasswordReset normalizes the email and emits PasswordResetOtpSent',
        () async {
      await cubit.requestPasswordReset(' Someone@Example.COM ');
      expect(repo.lastResetRequestedEmail, 'someone@example.com');
      expect(cubit.state, isA<PasswordResetOtpSent>());
      expect(
          (cubit.state as PasswordResetOtpSent).email, 'someone@example.com');
    });

    test(
        'requestPasswordReset rejects a malformed email without a network call',
        () async {
      await cubit.requestPasswordReset('not-an-email');
      expect(cubit.state, isA<AuthFailure>());
      expect(repo.lastResetRequestedEmail, isNull);
    });

    test('verifyPasswordResetOtp does nothing without a prior request',
        () async {
      await cubit.verifyPasswordResetOtp('123456');
      expect(repo.lastResetVerified, isNull);
    });

    test(
        'verifyPasswordResetOtp transitions to PasswordResetVerified on success',
        () async {
      repo.resetVerifyResult = const ResetAuthorizationResult(
          resetAuthorization: 'auth-secret', expiresInSeconds: 600);
      await cubit.requestPasswordReset('someone@example.com');
      await cubit.verifyPasswordResetOtp('482913');
      expect(repo.lastResetVerified?.email, 'someone@example.com');
      expect(repo.lastResetVerified?.code, '482913');
      expect(cubit.state, isA<PasswordResetVerified>());
    });

    test('completePasswordReset does nothing without a prior verification',
        () async {
      await cubit.completePasswordReset('NewPassw0rd!');
      expect(repo.lastResetCompleted, isNull);
    });

    test(
        'completePasswordReset sends the held authorization, never the OTP, '
        'and transitions to PasswordResetCompleted', () async {
      repo.resetVerifyResult = const ResetAuthorizationResult(
          resetAuthorization: 'auth-secret', expiresInSeconds: 600);
      await cubit.requestPasswordReset('someone@example.com');
      await cubit.verifyPasswordResetOtp('482913');
      await cubit.completePasswordReset('NewPassw0rd!');
      expect(repo.lastResetCompleted?.authorization, 'auth-secret');
      expect(repo.lastResetCompleted?.password, 'NewPassw0rd!');
      expect(cubit.state, isA<PasswordResetCompleted>());
    });

    test('a weak-password rejection surfaces as AuthFailure', () async {
      repo.resetVerifyResult = const ResetAuthorizationResult(
          resetAuthorization: 'auth-secret', expiresInSeconds: 600);
      repo.resetCompleteError =
          const ApiException(422, 'Choose a stronger password.');
      await cubit.requestPasswordReset('someone@example.com');
      await cubit.verifyPasswordResetOtp('482913');
      await cubit.completePasswordReset('weak');
      expect(cubit.state, isA<AuthFailure>());
    });

    test('restartPasswordReset clears held state and returns to SignedOut',
        () async {
      repo.resetVerifyResult = const ResetAuthorizationResult(
          resetAuthorization: 'auth-secret', expiresInSeconds: 600);
      await cubit.requestPasswordReset('someone@example.com');
      await cubit.verifyPasswordResetOtp('482913');
      cubit.restartPasswordReset();
      expect(cubit.state, isA<SignedOut>());
      // The cleared authorization can no longer be used.
      await cubit.completePasswordReset('NewPassw0rd!');
      expect(repo.lastResetCompleted, isNull);
    });
  });
}
