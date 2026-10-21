// Repository-level coverage for the Patient profile-photo feature
// (ProfileRepository.uploadPhoto/deletePhoto/fetchPhotoBytes), using the same
// Dio-interceptor request-capture style as doctor_client_test.dart /
// api_traffic_test.dart rather than a mocking library.
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/profile_repository.dart';

void main() {
  group('uploadPhoto', () {
    test('POSTs base64 content and mime type with no profile_id when self',
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
      final repo = ProfileRepository(ApiClient(dio: dio));
      final bytes = Uint8List.fromList([1, 2, 3, 4]);

      await repo.uploadPhoto(bytes: bytes, mimeType: 'image/jpeg');

      expect(sent.method, 'POST');
      expect(sent.path, '/patients/me/photo');
      final body = sent.data as Map;
      expect(body['mime_type'], 'image/jpeg');
      expect(base64Decode(body['base64_content'] as String), bytes);
      expect(body.containsKey('profile_id'), isFalse);
    });

    test('includes profile_id only when uploading for a dependent', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response(
            requestOptions: r,
            statusCode: 200,
            data: {'success': true, 'data': {}}));
      }));
      final repo = ProfileRepository(ApiClient(dio: dio));

      await repo.uploadPhoto(
        bytes: Uint8List.fromList([9]),
        mimeType: 'image/png',
        profileId: 'dep-1',
      );

      final body = sent.data as Map;
      expect(body['profile_id'], 'dep-1');
      expect(body['mime_type'], 'image/png');
    });
  });

  group('deletePhoto', () {
    test('DELETEs the photo endpoint with no query when self', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response(
            requestOptions: r,
            statusCode: 200,
            data: {'success': true, 'data': {}}));
      }));
      final repo = ProfileRepository(ApiClient(dio: dio));

      await repo.deletePhoto();

      expect(sent.method, 'DELETE');
      expect(sent.path, '/patients/me/photo');
      expect(sent.queryParameters.containsKey('profile_id'), isFalse);
    });

    test('passes profile_id as a query parameter for a dependent', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response(
            requestOptions: r,
            statusCode: 200,
            data: {'success': true, 'data': {}}));
      }));
      final repo = ProfileRepository(ApiClient(dio: dio));

      await repo.deletePhoto(profileId: 'dep-2');

      expect(sent.method, 'DELETE');
      expect(sent.queryParameters['profile_id'], 'dep-2');
    });
  });

  group('fetchPhotoBytes', () {
    test('returns the raw bytes from a successful GET', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      final payload = Uint8List.fromList([10, 20, 30, 40]);
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        expect(r.path, '/patients/me/photo');
        h.resolve(Response<List<int>>(
            requestOptions: r, statusCode: 200, data: payload));
      }));
      final repo = ProfileRepository(ApiClient(dio: dio));

      final bytes = await repo.fetchPhotoBytes();

      expect(bytes, payload);
    });

    test('sends profile_id as a query parameter for a dependent', () async {
      final dio = Dio(BaseOptions(baseUrl: 'http://localhost/api/v1'));
      late RequestOptions sent;
      final payload = Uint8List.fromList([1]);
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        sent = r;
        h.resolve(Response<List<int>>(
            requestOptions: r, statusCode: 200, data: payload));
      }));
      final repo = ProfileRepository(ApiClient(dio: dio));

      await repo.fetchPhotoBytes(profileId: 'dep-3');

      expect(sent.queryParameters['profile_id'], 'dep-3');
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
      final repo = ProfileRepository(ApiClient(dio: dio));

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
      final repo = ProfileRepository(ApiClient(dio: dio));

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
      final repo = ProfileRepository(ApiClient(dio: dio));

      await expectLater(
          repo.fetchPhotoBytes(),
          throwsA(isA<ApiException>().having((e) => e.status, 'status', null)));
    });
  });
}
