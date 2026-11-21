import 'dart:convert';
import 'dart:io';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/chat_live.dart';
import 'package:hellodoctor_mobile_patient/core/network/device_proof.dart';

class _Store implements TokenStore {
  final values = <String, String>{'access_token': 'token-1'};
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
  test(
      'connects with the session headers, relays nudges, '
      'and reconnects with a resync after the server closes', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final requests = <HttpHeaders>[];
    final paths = <String>[];
    server.listen((request) async {
      requests.add(request.headers);
      paths.add(request.uri.path);
      final socket = await WebSocketTransformer.upgrade(request);
      socket.add(jsonEncode({'type': 'ready'}));
      if (requests.length == 1) {
        socket.add(jsonEncode(
            {'type': 'chat', 'conversation_id': 'conv-1', 'kind': 'CHAT'}));
        // The real server closes every few minutes; the app must reconnect.
        await Future<void>.delayed(const Duration(milliseconds: 200));
        await socket.close(4000, 'reconnect');
      }
    });

    final api = ApiClient(
        dio:
            Dio(BaseOptions(baseUrl: 'http://127.0.0.1:${server.port}/api/v1')),
        tokenStore: _Store(),
        proofSigner: DeviceProofSigner(enabled: false));
    final live = ChatLive(api, observeLifecycle: false);
    final nudges = <ChatNudge>[];
    final sub = live.nudges.listen(nudges.add);
    live
      ..start()
      ..holdForTest();

    await _until(() => nudges.any((n) => n.conversationId == 'conv-1'));
    expect(live.connected.value, isTrue);
    expect(paths.first, '/api/v1/chat/live');
    expect(requests.first.value('authorization'), 'Bearer token-1');
    expect(requests.first.value('x-client-type'), 'PATIENT_FLUTTER');
    final nudge = nudges.firstWhere((n) => n.conversationId == 'conv-1');
    expect(nudge.concerns('conv-1'), isTrue);
    expect(nudge.concerns('conv-2'), isFalse);

    // After the close: a second connection and a resync for open screens.
    final resyncsBefore = nudges.where((n) => n.isResync).length;
    await _until(() => requests.length >= 2 && live.connected.value);
    await _until(() => nudges.where((n) => n.isResync).length > resyncsBefore);

    await live.stop();
    expect(live.connected.value, isFalse);
    await sub.cancel();
    await server.close(force: true);
  });

  test('a resync concerns every conversation', () {
    expect(const ChatNudge.resync().concerns('any'), isTrue);
  });
}

Future<void> _until(bool Function() condition,
    {Duration timeout = const Duration(seconds: 10)}) async {
  final end = DateTime.now().add(timeout);
  while (!condition()) {
    if (DateTime.now().isAfter(end)) fail('condition not met in $timeout');
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }
}
