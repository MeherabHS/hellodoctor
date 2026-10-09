import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';

/// Pins down the real-production-vs-LAN-test boundary that caused a release
/// build to white-screen on launch when its backend address wasn't HTTPS
/// (see app.dart / main.dart for the startup-crash-safety net this also
/// relies on). This policy must stay exactly this strict in both
/// directions: loosen it and a shipped build could send clinical data over
/// plain HTTP; tighten it and a correctly-configured field-test build breaks
/// again for every tester.
void main() {
  bool allows(String url,
          {bool flag = false, String environment = 'development'}) =>
      allowsInsecureHttp(
          allowInsecureHttpFlag: flag,
          baseUri: Uri.parse(url),
          environment: environment);

  test('production cannot enable LAN cleartext with a development flag', () {
    expect(
        allows('http://192.168.0.103:8080/api/v1',
            flag: true, environment: 'production'),
        isFalse);
    expect(
        allows('http://127.0.0.1:8080/api/v1',
            flag: true, environment: 'staging'),
        isFalse);
  });
  test(
      'release deployment rejects missing environment and all development flags in production',
      () {
    void validate(String environment,
            {bool http = false, bool otp = false, bool payout = false}) =>
        validateMobileDeployment(
            environment: environment,
            release: true,
            baseUri: Uri.parse('https://api.example.test/api/v1'),
            allowHttp: http,
            devOtp: otp,
            devPayoutBinding: payout);
    expect(() => validate(''), throwsStateError);
    expect(() => validate('unknown'), throwsStateError);
    expect(() => validate('production', http: true), throwsStateError);
    expect(() => validate('production', otp: true), throwsStateError);
    expect(() => validate('production', payout: true), throwsStateError);
    expect(() => validate('production'), returnsNormally);
  });
  test('development OTP requires both explicit development and its own flag',
      () {
    expect(
        developmentOtpEnabled(environment: 'production', flag: true), isFalse);
    expect(developmentOtpEnabled(environment: 'development', flag: false),
        isFalse);
    expect(
        developmentOtpEnabled(environment: 'development', flag: true), isTrue);
  });
  group('public hosts', () {
    test('https is always allowed', () {
      expect(allows('https://api.hellodoctor.asia/api/v1'), isTrue);
    });

    test('http on a public host is rejected without the explicit flag', () {
      expect(allows('http://api.hellodoctor.asia/api/v1'), isFalse);
      expect(allows('http://8.8.8.8/api/v1'), isFalse);
    });

    test('even the development flag cannot enable public cleartext traffic',
        () {
      expect(allows('http://api.hellodoctor.asia/api/v1', flag: true), isFalse);
    });
  });

  group('LAN / loopback test hosts', () {
    test('private and loopback addresses require the development flag', () {
      for (final host in [
        'http://10.0.2.2:8080/api/v1', // Android emulator host alias
        'http://10.1.2.3:8080/api/v1', // 10.0.0.0/8
        'http://192.168.0.103:8080/api/v1', // 192.168.0.0/16
        'http://172.16.0.1:8080/api/v1', // 172.16.0.0/12 lower bound
        'http://172.31.255.255:8080/api/v1', // 172.16.0.0/12 upper bound
        'http://127.0.0.1:8080/api/v1',
        'http://localhost:8080/api/v1',
      ]) {
        expect(allows(host), isFalse, reason: host);
        expect(allows(host, flag: true), isTrue, reason: host);
      }
    });

    test('addresses just outside the private ranges are not exempt', () {
      for (final host in [
        'http://172.15.255.255:8080/api/v1', // just below 172.16.0.0/12
        'http://172.32.0.0:8080/api/v1', // just above 172.16.0.0/12
        'http://11.0.0.1:8080/api/v1', // just above 10.0.0.0/8
        'http://192.169.0.1:8080/api/v1', // just above 192.168.0.0/16
      ]) {
        expect(allows(host), isFalse, reason: host);
      }
    });
  });
}
