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
        'payment-free CHAT: booking, chat waiting room and real repository chat at ${width}dp',
        (tester) async {
      tester.view.physicalSize = Size(width, 1000);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final bookings = <Map<String, dynamic>>[];
      final messages = <Map<String, dynamic>>[];
      final unexpected = <String>[];
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
        } else if (r.path.endsWith('/trace-ack')) {
          data = {}; // observational acknowledgement only
        } else if (r.path == '/config/booking-policy') {
          data = {'payment_required': false};
        } else if (r.path == '/appointments/book') {
          bookings.add(Map<String, dynamic>.from(r.data as Map));
          data = {
            'appointment_id': 'appointment',
            'appointment_number': 'HD1',
            'status': 'CONFIRMED',
            'payment_policy': 'NOT_REQUIRED',
            'amount': '0'
          };
        } else if (r.path == '/appointments/appointment') {
          data = {
            'id': 'appointment',
            'status': 'CONFIRMED',
            'payment_policy': 'NOT_REQUIRED',
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
          unexpected.add('${r.method} ${r.path}');
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
      void expectNoPaymentText() {
        for (final text in const [
          'KES',
          'Payment',
          'payment',
          'Paid',
          'M-Pesa'
        ]) {
          expect(find.textContaining(text), findsNothing, reason: text);
        }
      }

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
      expectNoPaymentText();
      await capture(tester, 'chat_details_${width.toInt()}');
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
      expectNoPaymentText();
      await capture(tester, 'chat_slot_booking_${width.toInt()}');
      await tester.tap(find.widgetWithText(FilledButton, 'Confirm Booking →'));
      await tester.pumpAndSettle();
      expect(bookings, hasLength(1));
      expect(bookings.single['modality'], 'CHAT');
      expect(bookings.single['slot_id'], 'slot');
      expect(bookings.single.containsKey('payment_method_id'), isFalse);
      expect(bookings.single.containsKey('gateway'), isFalse);
      expect(find.text('Consultation confirmed'), findsWidgets);
      expectNoPaymentText();
      await capture(tester, 'chat_confirmed_${width.toInt()}');
      await tester.scrollUntilVisible(find.text('Open chat waiting room'), 200,
          scrollable: find.byType(Scrollable).first);
      await tester.tap(find.text('Open chat waiting room'));
      await tester.pumpAndSettle();
      // Chat waiting room: no camera, microphone or video join.
      expect(find.text('Join Video Call'), findsNothing);
      expect(find.text('Camera'), findsNothing);
      await capture(tester, 'chat_waiting_room_${width.toInt()}');
      final openChat = find.widgetWithText(FilledButton, 'Open Chat');
      await tester.scrollUntilVisible(openChat, 200,
          scrollable: find.byType(Scrollable).first);
      await tester.tap(openChat);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'Hello doctor');
      await tester.testTextInput.receiveAction(TextInputAction.send);
      await tester.pumpAndSettle();
      expect(find.text('Hello doctor'), findsOneWidget);
      expect(messages, hasLength(1));
      expect(bookings, hasLength(1));
      expect(unexpected, isEmpty);
      expect(tester.takeException(), isNull);
      await capture(tester, 'patient_chat_${width.toInt()}');
      await tester.pumpWidget(const SizedBox.shrink());
      await family.close();
      api.dispose();
    });
  }
}
