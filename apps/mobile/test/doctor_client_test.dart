import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

void main() {
  test('prototype design tokens remain centralized', () {
    expect(DoctorTokens.primary.toARGB32(), 0xFF059669);
    expect(DoctorTokens.radius, 16);
  });

  test('domain error retains HTTP status for user mapping', () {
    const error =
        ApiException(409, 'Question is unavailable or already answered.');
    expect(error.status, 409);
    expect(error.message, contains('already answered'));
  });

  test('doctor state defaults to safe empty read state', () {
    const state = DoctorState();
    expect(state.queue, isEmpty);
    expect(state.slots, isEmpty);
    expect(state.duty, isFalse);
    expect(state.dutyUpdating, isFalse);
  });

  test('duty mutation state can disable repeated actions', () {
    const state = DoctorState(duty: true, dutyUpdating: true);
    expect(state.duty, isTrue);
    expect(state.dutyUpdating, isTrue);
  });

  test('updateSlotStatus PATCHes the slot with the requested status', () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    late RequestOptions sent;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      sent = r;
      h.resolve(Response(requestOptions: r, statusCode: 200, data: {
        'success': true,
        'data': {'slot_id': 's1', 'status': 'BLOCKED'}
      }));
    }));
    final repo = DoctorRepository(ApiClient(dio: dio));
    final result = await repo.updateSlotStatus('s1', 'BLOCKED');

    expect(sent.method, 'PATCH');
    expect(sent.path, '/doctor/slots/s1');
    expect(sent.data, {'status': 'BLOCKED'});
    expect(result['status'], 'BLOCKED');
  });

  test(
      'updateSlotStatus surfaces a 409 conflict as an ApiException the caller '
      'can catch, same as cancelSlot', () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      h.reject(DioException(
          requestOptions: r,
          type: DioExceptionType.badResponse,
          response: Response(requestOptions: r, statusCode: 409, data: {
            'success': false,
            'error': {
              'message':
                  'This slot cannot change availability. Refresh your schedule.'
            }
          })));
    }));
    final repo = DoctorRepository(ApiClient(dio: dio));
    await expectLater(repo.updateSlotStatus('s1', 'BLOCKED'),
        throwsA(isA<ApiException>().having((e) => e.status, 'status', 409)));
  });
}
