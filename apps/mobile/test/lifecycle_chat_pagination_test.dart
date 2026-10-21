import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

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
  test('Doctor latest 50 plus cursor pages expose all 200 persisted messages',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    final api = ApiClient(dio: dio, tokenStore: NoTokens());
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      var rows = List.generate(200, (i) => row(i + 1));
      if (r.queryParameters['before_id'] case final String before) {
        rows = rows
            .where((m) => (m['id'] as String).compareTo(before) < 0)
            .toList();
      }
      rows = rows.skip((rows.length - 50).clamp(0, rows.length)).toList();
      h.resolve(Response(
          requestOptions: r,
          statusCode: 200,
          data: {'success': true, 'data': rows}));
    }));
    final repo = DoctorRepository(api);
    final all = await repo.messages('conversation');
    expect(all.length, 50);
    expect(all.first.content, 'Message 151');
    expect(all.last.content, 'Message 200');
    for (var page = 0; page < 3; page++) {
      final older = await repo.olderMessages('conversation', all.first);
      expect(older.length, 50);
      all.insertAll(0, older);
    }
    expect(all.length, 200);
    expect(all.map((m) => m.id).toSet().length, 200);
    expect(all.first.content, 'Message 1');
    expect(await repo.olderMessages('conversation', all.first), isEmpty);
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
    final repo = DoctorRepository(api);
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
