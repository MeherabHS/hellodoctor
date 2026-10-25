import 'package:flutter/material.dart';
import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'doctor_widgets_test.dart' show repoHarness;

class ThreadRepository extends DoctorRepository {
  ThreadRepository() : super(ApiClient(dio: Dio()));
  @override
  Future<List<MessageDto>> messages(String id) async => const [];
}

AppointmentDto appointment(String modality, {String status = 'CONFIRMED'}) =>
    AppointmentDto.fromJson({
      'id': 'appointment',
      'patient_id': 'patient',
      'patient_display_name': 'Meherab Shafin',
      'modality': modality,
      'status': status,
      'conversation_id': 'shared-conversation',
    });
void main() {
  for (final modality in ['CHAT', 'VIDEO']) {
    testWidgets(
        '$modality thread exposes video control only for its paid modality',
        (tester) async {
      final chat = ChatDto(
          id: 'shared',
          appointmentId: 'appointment',
          patientName: 'Meherab Shafin',
          modality: modality,
          appointmentStatus: 'CONFIRMED',
          expiresAt: '2099-01-01T00:00:00Z',
          locked: false);
      await tester.pumpWidget(repoHarness(
          ChatThreadPage(chat: chat), const DoctorState(), ThreadRepository()));
      await tester.pumpAndSettle();
      expect(find.text('Meherab Shafin'), findsOneWidget);
      expect(find.byTooltip('Start Video Call'),
          modality == 'VIDEO' ? findsOneWidget : findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }
  test(
      'authoritative patient identity, modality and conversation survive mapping',
      () {
    final a = appointment('CHAT');
    expect(a.patientName, 'Meherab Shafin');
    expect(a.conversationId, 'shared-conversation');
    expect(a.isChat, isTrue);
    expect(a.startLabel, 'Start chat consultation');
    expect(appointment('VIDEO').startLabel, 'Start video consultation');
    expect(appointment('VIDEO', status: 'WAITING').canStart, isFalse);
    expect(appointment('CHAT', status: 'WAITING').canStart, isTrue);
    expect(
        AppointmentDto.fromJson({
          'id': 'a',
          'status': 'CONFIRMED',
          'modality': 'CHAT',
          'chat_locked': true
        }).canStart,
        isFalse);
    expect(
        AppointmentDto.fromJson({
          'id': 'a',
          'status': 'CONFIRMED',
          'modality': 'CHAT',
          'chat_expires_at': '2000-01-01T00:00:00Z'
        }).canStart,
        isFalse);
    for (final status in [
      'PENDING_PAYMENT',
      'CANCELLED',
      'REFUNDED',
      'COMPLETED',
      'EXPIRED',
      'PREMATURE_TERMINATION',
      'DISPUTED',
    ]) {
      expect(appointment('CHAT', status: status).canStart, isFalse);
      expect(appointment('VIDEO', status: status).canStart, isFalse);
    }
  });
  for (final modality in ['CHAT', 'VIDEO']) {
    testWidgets('$modality renders one modality-specific action at 320dp',
        (tester) async {
      tester.view.physicalSize = const Size(320, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(MaterialApp(
          home: Scaffold(body: AppointmentCard(appointment(modality)))));
      expect(find.text('Meherab Shafin'), findsOneWidget);
      expect(
          find.text(modality == 'CHAT'
              ? 'Start chat consultation'
              : 'Start video consultation'),
          findsOneWidget);
      expect(
          find.text(modality == 'CHAT'
              ? 'Start video consultation'
              : 'Start chat consultation'),
          findsNothing);
      expect(tester.takeException(), isNull);
    });
  }
  test(
      'conversation metadata controls closure and action without creating a new booking',
      () {
    final c = ChatDto.fromJson({
      'id': 'shared',
      'patient_display_name': 'Meherab Shafin',
      'appointment_id': 'a',
      'modality': 'CHAT',
      'appointment_status': 'CONFIRMED',
      'is_locked': false,
      'expires_at': '2099-01-01T00:00:00Z'
    });
    expect(c.patientName, 'Meherab Shafin');
    expect(c.isClosed, isFalse);
    expect(c.actionLabel, 'Start chat consultation');
    final cancelled = ChatDto.fromJson({
      'id': 'shared',
      'appointment_status': 'CANCELLED',
      'is_locked': false,
      'expires_at': '2099-01-01T00:00:00Z'
    });
    expect(cancelled.isClosed, isTrue);
    expect(cancelled.actionLabel, 'View consultation history');
    expect(
        const ChatDto(id: 'shared', expiresAt: '2000-01-01T00:00:00Z').isClosed,
        isTrue);
    for (final status in ['EXPIRED', 'PREMATURE_TERMINATION', 'DISPUTED']) {
      expect(
          ChatDto.fromJson({
            'id': 'shared',
            'appointment_status': status,
            'is_locked': false,
            'expires_at': '2099-01-01T00:00:00Z'
          }).isClosed,
          isTrue,
          reason: '$status must be treated as closed');
    }
  });
}
