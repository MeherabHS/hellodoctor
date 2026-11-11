import 'package:hellodoctor_mobile_patient/core/l10n/kenya_time.dart';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/paid_chat_booking_modal.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_models.dart';
import 'patient_discovery_qna_correction_test.dart'
    show MemoryTokens, host, doctorJson;
import 'chat_cubit_test.dart' show FakeChatRepository;

Future<void> capture(WidgetTester tester, String name) async {
  if (!const bool.fromEnvironment('CAPTURE_PATIENT_UI')) return;
  final boundary = tester
      .renderObject<RenderRepaintBoundary>(find.byKey(const Key('capture')));
  await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 2);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('../../docs/audit_evidence/patient_consultation/$name.png')
        .writeAsBytes(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

void main() {
  setUpAll(() async {
    await (FontLoader('Plus Jakarta Sans')
          ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf')))
        .load();
    await (FontLoader('MaterialIcons')
          ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf')))
        .load();
  });
  test(
      'a backend forbidden send closes the composer without losing unsent text/history',
      () async {
    final repo = FakeChatRepository()
      ..sendError = const ApiException(403, 'Consultation ended');
    final cubit = ChatCubit(repo, conversationId: 'shared');
    await cubit.send('Not sent');
    expect(cubit.state.notSent, isTrue);
    expect(cubit.state.serverLocked, isTrue);
    expect(repo.sendCallCount, 1);
    await cubit.send('Retry');
    expect(repo.sendCallCount, 1);
    await cubit.close();
  });
  for (final width in [320.0, 360.0, 390.0, 412.0]) {
    testWidgets(
        'paid CHAT chooser through booking, payment polling and real repository chat at ${width}dp',
        (tester) async {
      tester.view.physicalSize = Size(width, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      var paid = false;
      final bookings = <Map<String, dynamic>>[];
      final messages = <Map<String, dynamic>>[];
      final paymentMethods = <Map<String, dynamic>>[];
      final dio = Dio();
      dio.interceptors.add(InterceptorsWrapper(onRequest: (r, h) {
        dynamic data;
        if (r.path.endsWith('/slots')) {
          data = [
            {
              'id': 'slot',
              'doctor_id': 'doctor-test',
              'start_time': '2026-11-01T07:00:00Z',
              'end_time': '2026-11-01T07:10:00Z'
            }
          ];
        } else if (r.path == '/patients/me/payment-methods') {
          if (r.method == 'POST') {
            final method = {
              'id': 'pm-1',
              'gateway': 'MPESA',
              'masked_destination': 'M-Pesa •••• 5678',
              'is_primary': true,
            };
            paymentMethods.add(method);
            data = method;
          } else {
            data = paymentMethods;
          }
        } else if (r.path == '/appointments/book') {
          bookings.add(Map<String, dynamic>.from(r.data as Map));
          data = {
            'appointment_id': 'appointment',
            'appointment_number': 'HD1',
            'status': 'PENDING_PAYMENT',
            'amount': '350.00'
          };
        } else if (r.path == '/appointments/appointment') {
          data = {
            'id': 'appointment',
            'status': paid ? 'CONFIRMED' : 'PENDING_PAYMENT',
            'payment_status': paid ? 'PAYMENT_HELD' : 'INITIATED'
          };
        } else if (r.path.endsWith('/conversation')) {
          data = {
            'id': 'shared',
            'appointment_id': 'appointment',
            'is_locked': false,
            'expires_at': '2099-01-01T00:00:00Z'
          };
        } else if (r.path == '/chat/shared/messages') {
          if (r.method == 'POST') {
            final message = {
              'id': 'message',
              'sender_id': 'patient',
              'content': r.data['content'],
              'sent_at': '2026-10-14T07:00:00Z'
            };
            messages.add(message);
            data = message;
          } else {
            data = messages;
          }
        } else {
          h.reject(DioException(
              requestOptions: r, error: 'Unexpected request ${r.path}'));
          return;
        }
        h.resolve(Response(
            requestOptions: r,
            data: {'success': true, 'data': data},
            statusCode: r.method == 'POST' ? 201 : 200));
      }));
      final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
      final repo = BookingRepository(api);
      final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
      await tester.pumpWidget(host(Builder(
          builder: (context) => Scaffold(
              body: FilledButton(
                  onPressed: () => showPaidChatBookingModal(context,
                      doctor: PublicDoctorDto.fromJson(doctorJson),
                      repo: repo,
                      familyProfilesCubit: family),
                  child: const Text('Book chat'))))));
      await tester.tap(find.text('Book chat'));
      await tester.pumpAndSettle();
      await capture(tester, 'paid_chat_details_${width.toInt()}');
      await tester
          .tap(find.widgetWithText(FilledButton, 'Select a consultation slot'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('1').first);
      await tester.pumpAndSettle();
      final local = kenyaTime(DateTime.parse('2026-11-01T07:00:00Z'));
      final hour = local.hour % 12 == 0 ? 12 : local.hour % 12;
      final label =
          '${hour.toString().padLeft(2, '0')}:00 ${local.hour >= 12 ? 'PM' : 'AM'}';
      await tester.tap(find.text(label));
      await tester.pumpAndSettle();
      await capture(tester, 'chat_slot_booking_${width.toInt()}');
      await tester.tap(find.text('+ Add M-Pesa Number'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField).last, '0712345678');
      await tester.tap(find.text('Save Payment Method'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(FilledButton).last);
      await tester.pumpAndSettle();
      expect(bookings, hasLength(1));
      expect(bookings.single['modality'], 'CHAT');
      expect(bookings.single['slot_id'], 'slot');
      expect(bookings.single['payment_method_id'], 'pm-1');
      expect(find.text('Payment required'), findsOneWidget);
      expect(find.text('Open consultation'), findsNothing);
      expect(
          tester
              .widget<FilledButton>(find.widgetWithText(
                  FilledButton, 'Proceed to payment method'))
              .onPressed,
          isNotNull);
      await capture(tester, 'chat_payment_pending_${width.toInt()}');
      paid = true;
      await tester.tap(find.text('Refresh status'));
      await tester.pumpAndSettle();
      expect(find.text('Consultation confirmed'), findsOneWidget);
      await capture(tester, 'chat_payment_confirmed_${width.toInt()}');
      await tester.tap(find.text('Open consultation'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Hello doctor');
      await tester.testTextInput.receiveAction(TextInputAction.send);
      await tester.pumpAndSettle();
      expect(find.text('Hello doctor'), findsOneWidget);
      expect(messages, hasLength(1));
      expect(bookings, hasLength(1));
      expect(find.textContaining('Upcoming feature'), findsNothing);
      expect(tester.takeException(), isNull);
      await capture(tester, 'patient_chat_${width.toInt()}');
      await tester.pumpWidget(const SizedBox.shrink());
      await family.close();
      api.dispose();
    });
  }
}
