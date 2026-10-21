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
}
