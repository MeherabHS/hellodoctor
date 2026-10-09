import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

class _MemoryTokenStore implements TokenStore {
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

/// Minimal fake transport: every request succeeds with an empty `data`
/// payload, which is all `ConsultationPage.load()` needs to settle.
class _FakeAdapter implements HttpClientAdapter {
  @override
  Future<ResponseBody> fetch(RequestOptions options,
      Stream<Uint8List>? requestStream, Future<void>? cancelFuture) async {
    final body = utf8.encode(jsonEncode({'success': true, 'data': {}}));
    return ResponseBody.fromBytes(body, 200, headers: {
      Headers.contentTypeHeader: [Headers.jsonContentType]
    });
  }

  @override
  void close({bool force = false}) {}
}

void main() {
  testWidgets(
      'consultation completion dialog offers all four clinical outcomes, '
      'including Escalate (CLAUDE.md Business Rule 6)', (tester) async {
    final dio = Dio()..httpClientAdapter = _FakeAdapter();
    final api = ApiClient(dio: dio, tokenStore: _MemoryTokenStore());
    final repo = DoctorRepository(api);
    final cubit = DoctorCubit(repo);

    final appointment = AppointmentDto.fromJson({
      'id': 'apt-1',
      'status': 'IN_PROGRESS',
      'doctor_lifecycle': 'IN_PROGRESS',
      'patient_display_name': 'Test patient',
    });

    await tester.pumpWidget(BlocProvider.value(
        value: cubit,
        child: MaterialApp(home: ConsultationPage(appointment: appointment))));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Complete consultation'));
    await tester.pumpAndSettle();

    expect(find.text('Escalate'), findsOneWidget);
    expect(find.text('Refer patient'), findsOneWidget);
    expect(find.text('Complete without Rx'), findsOneWidget);
    expect(find.text('Complete with Rx'), findsOneWidget);
  });
}
