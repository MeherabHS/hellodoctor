// Renders the Smart Rx screens with the real fonts, for comparing against
// docs/design/doctor-ui/smart-rx-dispatch-flow.png. Skipped unless asked:
//   RX_SHOTS=<output folder> flutter test test/smart_rx_screenshots_test.dart
import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

final _out = Platform.environment['RX_SHOTS'];

class _QuietNotifications extends NotificationCubit {
  _QuietNotifications(super.repo);
  @override
  Future<void> load() async {}
}

Future<void> _loadFont(String family, String path) async {
  final loader = FontLoader(family)
    ..addFont(Future.value(ByteData.sublistView(File(path).readAsBytesSync())));
  await loader.load();
}

AppointmentDto _apt(String id, String name, String status,
        {String? outcome,
        DateTime? completed,
        Duration ago = Duration.zero,
        bool chat = false}) =>
    AppointmentDto(
        id: id,
        number: 'AM-$id',
        status: status,
        lifecycle: status,
        patientName: name,
        modality: chat ? 'CHAT' : 'VIDEO',
        clinicalOutcome: outcome,
        completedAt: completed?.toIso8601String(),
        end: completed,
        start: DateTime.now().toUtc().subtract(ago));

Future<void> _shot(WidgetTester tester, GlobalKey key, String name) async {
  for (var i = 0; i < 8; i++) {
    await tester.pump(const Duration(milliseconds: 100));
  }
  final boundary =
      key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
  final bytes = await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 2);
    final data = await image.toByteData(format: ui.ImageByteFormat.png);
    return data!.buffer.asUint8List();
  });
  File('$_out/$name.png').writeAsBytesSync(bytes!);
}

void main() {
  final skip = _out == null ? 'set RX_SHOTS to render screenshots' : null;

  setUpAll(() async {
    if (_out == null) return;
    Directory(_out!).createSync(recursive: true);
    await _loadFont('Plus Jakarta Sans', 'assets/fonts/PlusJakartaSans.ttf');
    await _loadFont(
        'MaterialIcons',
        '/c/Users/CUBE/tools/flutter/bin/cache/artifacts/material_fonts/materialicons-regular.otf'
            .replaceFirst('/c/', 'C:/'));
  });

  testWidgets('render Smart Rx screens', skip: skip != null, (tester) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final now = DateTime.now().toUtc();
    final queue = [
      _apt('784532', 'John Mwangi', 'IN_CONSULTATION',
          ago: const Duration(minutes: 20)),
      _apt('784533', 'Amina Njeri', 'WAITING', chat: true),
      _apt('784534', 'Peter Kariuki', 'CONFIRMED',
          ago: const Duration(hours: 1)),
      _apt('784535', 'Sarah Nditiru', 'COMPLETED',
          outcome: 'REFERRED',
          completed: now.subtract(const Duration(hours: 2)),
          ago: const Duration(hours: 3),
          chat: true),
      _apt('784536', 'David Kimani', 'COMPLETED',
          outcome: 'COMPLETED_WITH_RX',
          completed: now.subtract(const Duration(hours: 20)),
          ago: const Duration(hours: 21)),
    ];
    final repo = StubDoctorRepository()
      ..queueResponse = (Completer<List<AppointmentDto>>()..complete(queue));
    final cubit = DoctorCubit(repo);
    addTearDown(cubit.close);
    await cubit.loadAll();
    final notifications =
        _QuietNotifications(NotificationRepository(ApiClient(dio: Dio())));
    addTearDown(notifications.close);
    final key = GlobalKey();
    Future<void> show(Widget page) async {
      await tester.pumpWidget(RepaintBoundary(
          key: key,
          child: MultiBlocProvider(
              providers: [
                BlocProvider<DoctorCubit>.value(value: cubit),
                BlocProvider<NotificationCubit>.value(value: notifications),
              ],
              child: MaterialApp(
                  debugShowCheckedModeBanner: false,
                  theme: DoctorTokens.theme(),
                  home: page))));
      for (var i = 0; i < 8; i++) {
        await tester.pump(const Duration(milliseconds: 100));
      }
    }

    // 1. Picker (completed consultations only) with Sarah selected.
    await show(const ScannerPage());
    await tester.tap(find.text('Sarah Nditiru'));
    await _shot(tester, key, '1-select-consultation');

    // 2. Capture step.
    await tester.tap(find.widgetWithText(FilledButton, 'Continue'));
    await _shot(tester, key, '2-capture');

    // 2b. Page strip in every state.
    await show(Scaffold(
        backgroundColor: DoctorTokens.bg,
        body: Padding(
            padding: const EdgeInsets.fromLTRB(18, 60, 18, 18),
            child: rxPageSlotsPreview(
                ['done', 'uploading', 'failed', 'pending', null],
                progress: .6))));
    await _shot(tester, key, '2b-page-states');

    // 3. Offline.
    repo.offlineFlag = true;
    await show(ScannerPage(initialAppointment: queue.first));
    await _shot(tester, key, '3-offline');
    repo.offlineFlag = false;

    // 4. Submitted.
    await show(Scaffold(
        backgroundColor: const Color(0x99000000),
        body: Center(
            child: rxSubmittedPreview(
                patient: 'John Mwangi', pages: 4, at: DateTime.now()))));
    await _shot(tester, key, '4-submitted');
  });
}
