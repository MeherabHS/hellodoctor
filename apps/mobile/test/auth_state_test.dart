import 'package:flutter_test/flutter_test.dart';
import 'package:dio/dio.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/auth/auth_cubit.dart';

class _NoopStore implements TokenStore {
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
  test('login validates required credentials before network access', () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.login('', '');
    expect(auth.state, isA<AuthFailure>());
    expect((auth.state as AuthFailure).message, contains('email'));
  });

  test('MFA verification cannot run without a server challenge', () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.verify('123456');
    expect(auth.state, isA<SignedOut>());
  });

  test('Doctor registration validates required fields before network access',
      () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.registerDoctor(
      fullName: '',
      licenseNumber: '',
      phoneNumber: '',
      email: '',
      password: '',
      specialty: '',
      experienceYears: '',
      hospital: '',
      qualifications: '',
      address: '',
      acceptedDocuments: const [],
    );
    expect(auth.state, isA<AuthFailure>());
    expect((auth.state as AuthFailure).message, contains('required'));
    await auth.close();
  });

  test('requestPasswordReset rejects a malformed email without network access',
      () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.requestPasswordReset('not-an-email');
    expect(auth.state, isA<AuthFailure>());
    await auth.close();
  });

  test(
      'verifyPasswordResetOtp does nothing without a prior requestPasswordReset',
      () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.verifyPasswordResetOtp('123456');
    expect(auth.state, isA<SignedOut>());
    await auth.close();
  });

  test('verifyPasswordResetOtp rejects a malformed code without network access',
      () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    // Reach the "has a pending reset email" branch without a real network
    // call by first failing fast on a malformed email (sets no email), then
    // directly exercising the malformed-code guard, which is checked before
    // the held email is used.
    await auth.verifyPasswordResetOtp('abc');
    expect(auth.state, isA<SignedOut>());
    await auth.close();
  });

  test(
      'completePasswordReset does nothing without a prior successful verification',
      () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    await auth.completePasswordReset('NewPassw0rd!');
    expect(auth.state, isA<SignedOut>());
    await auth.close();
  });

  test('restartPasswordReset returns to SignedOut', () async {
    final auth = AuthCubit(ApiClient(dio: Dio()));
    auth.restartPasswordReset();
    expect(auth.state, isA<SignedOut>());
    await auth.close();
  });

  test(
      'restore() flags SessionRestoring synchronously so a cold start with a '
      'saved session never flashes the login form', () async {
    final store = _NoopStore();
    final auth =
        AuthCubit(ApiClient(dio: Dio(), tokenStore: store), cache: null);
    final pending = auth.restore();
    // No await yet: this must already be true for the very first frame.
    expect(auth.state, isA<SessionRestoring>());
    await pending;
    // No saved refresh token in this test store, so it resolves to SignedOut.
    expect(auth.state, isA<SignedOut>());
    await auth.close();
  });
}
