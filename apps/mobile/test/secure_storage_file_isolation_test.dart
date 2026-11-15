import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';

/// Regression for the intermittent post-login sign-out (2026-11-13).
///
/// flutter_secure_storage 9.x on Android keeps one native instance and never
/// resets its preferences file name to the default once a call supplies one.
/// A store that omits the name therefore inherits whichever file was used last
/// (e.g. the boot diagnostics file), and session tokens read back as null. Every
/// store must name its file explicitly.
void main() {
  test('session token store names the default preferences file', () {
    final options = SecureTokenStore().storage.aOptions.toMap();
    expect(options['sharedPreferencesName'], sessionSecureStorageFile);
    expect(sessionSecureStorageFile, 'FlutterSecureStorage',
        reason: 'renaming the file would orphan every existing session');
  });

  test('every FlutterSecureStorage in lib/ names its preferences file', () {
    final offenders = <String>[];
    for (final file in Directory('lib')
        .listSync(recursive: true)
        .whereType<File>()
        .where((f) => f.path.endsWith('.dart'))) {
      final source = file.readAsStringSync();
      for (final match
          in RegExp(r'FlutterSecureStorage\(').allMatches(source)) {
        final call = _balancedCall(source, match.end - 1);
        if (!call.contains('sharedPreferencesName')) {
          offenders.add('${file.path}:${_line(source, match.start)}');
        }
      }
    }
    expect(offenders, isEmpty,
        reason: 'unnamed secure storage shares a sticky native file name');
  });
}

String _balancedCall(String source, int openParen) {
  var depth = 0;
  for (var i = openParen; i < source.length; i++) {
    if (source[i] == '(') depth++;
    if (source[i] == ')' && --depth == 0) {
      return source.substring(openParen, i + 1);
    }
  }
  return source.substring(openParen);
}

int _line(String source, int offset) =>
    '\n'.allMatches(source.substring(0, offset)).length + 1;
