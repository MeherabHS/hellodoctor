import 'dart:io';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/patient_booking_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'patient_discovery_qna_correction_test.dart'
    show MemoryTokens, host, doctorJson;

/// Set `--dart-define=OUT_DIR=<dir>` to also save PNG screenshots.
const _out = String.fromEnvironment('OUT_DIR');
const _width = int.fromEnvironment('WIDTH', defaultValue: 360);

/// Prototype `view-4` (index.html "Scheduled Video Consultation") parity: the
/// same booking steps must show whether payment is required or deferred; only
/// the payment itself (method picker / charge) is switched off by policy.
void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')
          ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf')))
        .load();
    await (FontLoader('MaterialIcons')
          ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf')))
        .load();
  });

  Future<void> pump(WidgetTester tester,
      {required bool paymentRequired,
      required String fee,
      double? width}) async {
    width ??= _width.toDouble();
    tester.view.physicalSize = Size(width, 1500);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final dio = Dio();
    final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
    addTearDown(api.dispose);
    final connectivity = ConnectivityService();
    addTearDown(connectivity.dispose);
    final now = DateTime.now().toUtc().add(const Duration(days: 1));
    final base = DateTime.utc(now.year, now.month, now.day, 7, 0);
    dio.interceptors.add(InterceptorsWrapper(onRequest: (o, h) {
      dynamic data;
      if (o.path.endsWith('/slots')) {
        data = [
          for (var i = 0; i < 6; i++)
            {
              'id': 's$i',
              'doctor_id': 'doctor-test',
              'start_time':
                  base.add(Duration(minutes: 10 * i)).toIso8601String(),
              'end_time':
                  base.add(Duration(minutes: 10 * i + 10)).toIso8601String(),
            }
        ];
      } else if (o.path.endsWith('/config/booking-policy')) {
        data = {'payment_required': paymentRequired};
      } else {
        data = [];
      }
      h.resolve(Response(
          requestOptions: o,
          statusCode: 200,
          data: {'success': true, 'data': data}));
    }));
    final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
    addTearDown(family.close);
    final doctor = PublicDoctorDto.fromJson({
      ...doctorJson,
      'consultation_fee_video': fee,
      'consultation_fee_chat': '0'
    });
    await tester.pumpWidget(host(RepaintBoundary(
        key: const Key('cap'),
        child: PatientBookingPage(
            doctor: doctor,
            modality: 'VIDEO',
            repo: BookingRepository(api, connectivity: connectivity),
            familyProfilesCubit: family))));
    await tester.pumpAndSettle();
  }

  Future<void> shot(WidgetTester tester, String name) async {
    if (_out.isEmpty) return;
    final b = tester
        .renderObject<RenderRepaintBoundary>(find.byKey(const Key('cap')));
    await tester.runAsync(() async {
      final img = await b.toImage(pixelRatio: 2);
      final bytes = await img.toByteData(format: ui.ImageByteFormat.png);
      await File('$_out/$name.png').writeAsBytes(bytes!.buffer.asUint8List());
    });
  }

  testWidgets('deferred mode shows every booking step, no payment picker',
      (tester) async {
    await pump(tester, paymentRequired: false, fee: '0');
    expect(find.text('Scheduled Video Consultation'), findsOneWidget);
    expect(find.text('Select Date'), findsOneWidget);
    expect(find.text('Available Slots'), findsOneWidget);
    expect(find.text('Times shown in EAT (UTC+3)'), findsOneWidget);
    expect(find.text('Consultation Fee (10 mins)'), findsOneWidget);
    expect(find.text('24-Hour Dedicated Chat'), findsOneWidget);
    expect(find.text('Payable now'), findsOneWidget);
    expect(find.textContaining('Payment deferred'), findsOneWidget);
    expect(find.textContaining('24-Hour Chat Included:'), findsOneWidget);
    expect(find.text('Total Payable'), findsNothing);
    expect(find.textContaining('M-Pesa'), findsNothing);
    await tester.tap(find.text('10:00 AM').first);
    await tester.pumpAndSettle();
    expect(
        find.widgetWithText(FilledButton, 'Confirm Booking →'), findsOneWidget);
    await shot(tester, 'deferred_booking_$_width');
  });

  testWidgets('paid mode keeps the same steps plus total and payment picker',
      (tester) async {
    await pump(tester, paymentRequired: true, fee: '630');
    expect(find.text('Scheduled Video Consultation'), findsOneWidget);
    expect(find.text('Total Payable'), findsOneWidget);
    expect(find.text('KES 630'), findsWidgets);
    expect(find.text('Payable now'), findsNothing);
    expect(find.textContaining('24-Hour Chat Included:'), findsOneWidget);
    await shot(tester, 'paid_booking_$_width');
  });

  List<String> chipLabels(WidgetTester tester) => tester
      .widgetList<Text>(find.byType(Text))
      .map((t) => t.data ?? '')
      .where((t) => RegExp(r'^\d{2}:\d{2} [AP]M$').hasMatch(t))
      .toList();

  testWidgets('slot times are identical in paid and deferred mode',
      (tester) async {
    await pump(tester, paymentRequired: false, fee: '0');
    final deferred = chipLabels(tester);
    await pump(tester, paymentRequired: true, fee: '630');
    final paid = chipLabels(tester);
    // 07:00Z + 10-minute steps == 10:00 AM EAT onward, same in both modes.
    expect(deferred, [
      '10:00 AM',
      '10:10 AM',
      '10:20 AM',
      '10:30 AM',
      '10:40 AM',
      '10:50 AM'
    ]);
    expect(paid, deferred);
  });
}
