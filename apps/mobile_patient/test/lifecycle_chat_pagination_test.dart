import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_cubit.dart';

class NoTokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

Map<String, dynamic> row(int i) => {
      'id': '00000000-0000-4000-8000-${i.toString().padLeft(12, '0')}',
      'sender_id': 'doctor',
      'content': 'Message $i',
      'sent_at':
          DateTime.utc(2026, 10, 16).add(Duration(seconds: i)).toIso8601String()
    };
void main() {
  test(
      '200 messages remain reachable while a newer message arrives during older pagination',
      () async {
    var total = 200;
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final api = ApiClient(dio: dio, tokenStore: NoTokens());
    final olderRequested = Completer<RequestOptions>();
    late RequestInterceptorHandler olderHandler;
    final repo = ChatRepository(api);
    final cubit = ChatCubit(repo, conversationId: 'conversation');
    addTearDown(cubit.close);
    var holdOlder = true;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      if (r.queryParameters.containsKey('before_id') && holdOlder) {
        olderHandler = h;
        olderRequested.complete(r);
        return;
      }
      var rows = List.generate(total, (i) => row(i + 1));
      if (r.queryParameters['after_id'] case final String after) {
        rows = rows
            .where((m) => (m['id'] as String).compareTo(after) > 0)
            .take(50)
            .toList();
      } else if (r.queryParameters['before_id'] case final String before) {
        rows = rows
            .where((m) => (m['id'] as String).compareTo(before) < 0)
            .toList();
        rows = rows.skip((rows.length - 50).clamp(0, rows.length)).toList();
      } else {
        rows = rows.skip((rows.length - 50).clamp(0, rows.length)).toList();
      }
      h.resolve(Response(
          requestOptions: r,
          statusCode: 200,
          data: {'success': true, 'data': rows}));
    }));
    await cubit.load();
    expect(cubit.state.messages.length, 50);
    expect(cubit.state.messages.first.content, 'Message 151');
    expect(cubit.state.messages.last.content, 'Message 200');
    final older = cubit.loadOlder();
    final request = await olderRequested.future;
    total = 201;
    await cubit.poll();
    olderHandler.resolve(Response(
        requestOptions: request,
        statusCode: 200,
        data: {
          'success': true,
          'data': List.generate(50, (i) => row(i + 101))
        }));
    await older;
    expect(cubit.state.messages.last.content, 'Message 201');
    expect(cubit.state.messages.length, 101);
    holdOlder = false;
    await cubit.loadOlder();
    await cubit.loadOlder();
    expect(cubit.state.messages.length, 201);
    expect(cubit.state.messages.map((m) => m.id).toSet().length, 201);
    expect(cubit.state.messages.first.content, 'Message 1');
    await cubit.loadOlder();
    expect(cubit.state.hasOlder, isFalse);
  });
  test(
      'uncertain send retry retains its key; next intentional identical send has a new key',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final api = ApiClient(dio: dio, tokenStore: NoTokens());
    final keys = <String>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      keys.add(r.headers['Idempotency-Key'] as String);
      if (keys.length == 1) {
        h.reject(DioException(
            requestOptions: r, type: DioExceptionType.connectionError));
        return;
      }
      h.resolve(Response(
          requestOptions: r,
          statusCode: 201,
          data: {'success': true, 'data': row(keys.length)}));
    }));
    final repo = ChatRepository(api);
    await expectLater(
        repo.sendMessage('conversation', 'Identical intentional text'),
        throwsA(isA<ApiException>()));
    await repo.sendMessage('conversation', 'Identical intentional text');
    await repo.sendMessage('conversation', 'Identical intentional text');
    expect(keys.length, 3);
    expect(keys[0], keys[1]);
    expect(keys[2], isNot(keys[1]));
  });
}
