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
      'bindPayoutDestination posts the real binding endpoint, not the legacy '
      'payout-method pair', () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    late RequestOptions sent;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      sent = r;
      h.resolve(Response(requestOptions: r, statusCode: 200, data: {
        'success': true,
        'data': {
          'id': 'dest-1',
          'provider_code': 'MPESA',
          'method_type': 'MPESA',
          'masked_destination': 'MPESA •••• 5678',
          'binding_status': 'ACTIVE',
          'verification_status': 'AWAITING_PROVIDER_INTEGRATION',
          'payout_capable': true,
          'is_primary': true,
        }
      }));
    }));
    final repo = DoctorRepository(ApiClient(dio: dio));
    final result =
        await repo.bindPayoutDestination('+254712345678', makePrimary: true);

    expect(sent.method, 'POST');
    expect(sent.path, '/doctor/wallet/payout-methods/bind');
    expect(sent.data, {
      'method_type': 'MPESA',
      'account_identifier': '+254712345678',
      'make_primary': true,
    });
    expect(result['id'], 'dest-1');
    expect(result['masked_destination'], 'MPESA •••• 5678');
  });

  test(
      'bindPayoutDestination surfaces the honest 409 when no platform provider is configured',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      h.reject(DioException(
          requestOptions: r,
          type: DioExceptionType.badResponse,
          response: Response(requestOptions: r, statusCode: 409, data: {
            'success': false,
            'error': {
              'message':
                  'The platform has not configured a MPESA payment provider yet.'
            }
          })));
    }));
    final repo = DoctorRepository(ApiClient(dio: dio));
    await expectLater(
        repo.bindPayoutDestination('+254712345678'),
        throwsA(isA<ApiException>()
            .having((e) => e.status, 'status', 409)
            .having((e) => e.userMessage, 'userMessage',
                contains('has not configured'))));
  });

  test('setPrimaryPayoutDestination posts the real set-primary endpoint',
      () async {
    final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
    late RequestOptions sent;
    dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
      sent = r;
      h.resolve(Response(requestOptions: r, statusCode: 200, data: {
        'success': true,
        'data': {'id': 'dest-2', 'is_primary': true}
      }));
    }));
    final repo = DoctorRepository(ApiClient(dio: dio));
    await repo.setPrimaryPayoutDestination('dest-2');

    expect(sent.method, 'POST');
    expect(sent.path, '/doctor/wallet/payout-methods/set-primary');
    expect(sent.data, {'destination_id': 'dest-2'});
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
