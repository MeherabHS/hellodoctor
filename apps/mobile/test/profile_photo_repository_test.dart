// Repository-level coverage for the Doctor profile-photo feature
// (DoctorRepository.uploadPhoto/deletePhoto/fetchPhotoBytes), using the same
// Dio-interceptor request-capture style already established by
// `doctor_client_test.dart`.
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

void main() {
  group('uploadPhoto', () {
    test('POSTs base64 content and mime type to /doctor/profile/photo',
        () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response(
            requestOptions: r,
            statusCode: 200,
            data: {'success': true, 'data': {}}));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));
      final bytes = Uint8List.fromList([1, 2, 3, 4]);

      await repo.uploadPhoto(bytes: bytes, mimeType: 'image/jpeg');

      expect(sent.method, 'POST');
      expect(sent.path, '/doctor/profile/photo');
      final body = sent.data as Map;
      expect(body['mime_type'], 'image/jpeg');
      expect(base64Decode(body['base64_content'] as String), bytes);
      expect(body.containsKey('profile_id'), isFalse,
          reason: 'a doctor has no dependent-profile concept');
    });
  });

  group('deletePhoto', () {
    test('DELETEs /doctor/profile/photo with no query', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response(
            requestOptions: r,
            statusCode: 200,
            data: {'success': true, 'data': {}}));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));

      await repo.deletePhoto();

      expect(sent.method, 'DELETE');
      expect(sent.path, '/doctor/profile/photo');
      expect(sent.queryParameters, isEmpty);
    });
  });

  group('fetchPhotoBytes', () {
    test('returns the raw bytes from a successful GET', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      final payload = Uint8List.fromList([10, 20, 30, 40]);
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        expect(r.path, '/doctor/profile/photo');
        h.resolve(Response<List<int>>(
            requestOptions: r, statusCode: 200, data: payload));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));

      final bytes = await repo.fetchPhotoBytes();

      expect(bytes, payload);
    });

    test('returns null (not a thrown error) on a 404 — no photo set',
        () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        h.reject(DioException(
            requestOptions: r,
            type: DioExceptionType.badResponse,
            response: Response(
                requestOptions: r,
                statusCode: 404,
                data: {
                  'success': false,
                  'error': {'message': 'Not found.'}
                })));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));

      final bytes = await repo.fetchPhotoBytes();

      expect(bytes, isNull);
    });

    test('a 500 still propagates as a thrown ApiException', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        h.reject(DioException(
            requestOptions: r,
            type: DioExceptionType.badResponse,
            response: Response(
                requestOptions: r,
                statusCode: 500,
                data: {
                  'success': false,
                  'error': {'message': 'Server error.'}
                })));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));

      await expectLater(
          repo.fetchPhotoBytes(),
          throwsA(
              isA<ApiException>().having((e) => e.status, 'status', 500)));
    });

    test('a network error still propagates as a thrown ApiException',
        () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        h.reject(DioException(
            requestOptions: r, type: DioExceptionType.connectionError));
      }));
      final repo = DoctorRepository(ApiClient(dio: dio));

      await expectLater(
          repo.fetchPhotoBytes(),
          throwsA(isA<ApiException>().having((e) => e.status, 'status', null)));
    });
  });
}
