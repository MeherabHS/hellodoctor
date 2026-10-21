import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/app.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

import 'doctor_cubit_test.dart' show StubDoctorRepository;

class LifecycleQueueRepository extends StubDoctorRepository {
  int queueReads = 0;
  List<AppointmentDto> current = [
    const AppointmentDto(
      id: 'stale',
      patientName: 'Previous Patient',
      status: 'CONFIRMED',
      lifecycle: 'UPCOMING',
      queueActionable: true,
    ),
  ];
  @override
  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async {
    queueReads++;
    return offset == 0 ? current : [];
  }
}

class QuietNotifications extends NotificationCubit {
  QuietNotifications(super.repo);
  @override
  Future<void> load() async {}
}

Future<void> mount(
  WidgetTester tester,
  DoctorCubit doctor,
  NotificationCubit notifications,
) async {
  await tester.pumpWidget(
    MultiBlocProvider(
      providers: [
        BlocProvider<DoctorCubit>.value(value: doctor),
        BlocProvider<NotificationCubit>.value(value: notifications),
      ],
      child: const MaterialApp(home: DoctorShell()),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets(
    'resume, Queue return and foreground tick replace stale operational data; background does not poll',
    (tester) async {
      final repo = LifecycleQueueRepository();
      final doctor = DoctorCubit(repo);
      final api = ApiClient(dio: Dio());
      final notifications = QuietNotifications(NotificationRepository(api));
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
        await doctor.close();
        await notifications.close();
        api.dispose();
      });
      await mount(tester, doctor, notifications);
      expect(doctor.state.queue.single.id, 'stale');
      final firstReads = repo.queueReads;
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
      repo.current = [];
      await tester.pump(const Duration(seconds: 31));
      expect(repo.queueReads, firstReads);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      // pumpAndSettle does not advance a timer that has not scheduled a frame.
      // Resume deliberately waits 250–2000ms; advance its documented upper
      // bound in fake time, then require exactly one authoritative queue read.
      await tester.pump(const Duration(seconds: 2));
      await tester.pumpAndSettle();
      expect(repo.queueReads, firstReads + 1);
      expect(doctor.state.queue, isEmpty);
      expect(find.text('Previous Patient'), findsNothing);
      repo.current = [
        const AppointmentDto(
          id: 'next',
          patientName: 'Next Patient',
          status: 'CONFIRMED',
          lifecycle: 'UPCOMING',
        ),
      ];
      await tester.pump(const Duration(seconds: 30));
      await tester.pumpAndSettle();
      expect(doctor.state.queue.single.id, 'next');
      final nav = find.byType(DoctorBottomNav);
      await tester.tap(find.descendant(of: nav, matching: find.text('Chat')));
      await tester.pumpAndSettle();
      repo.current = [];
      await tester.tap(find.descendant(of: nav, matching: find.text('Queue')));
      await tester.pumpAndSettle();
      expect(doctor.state.queue, isEmpty);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('rapid resumes coalesce and disposal cancels pending refresh', (
    tester,
  ) async {
    final repo = LifecycleQueueRepository();
    final doctor = DoctorCubit(repo);
    final api = ApiClient(dio: Dio());
    final notifications = QuietNotifications(NotificationRepository(api));
    addTearDown(() async {
      await tester.pumpWidget(const SizedBox.shrink());
      await doctor.close();
      await notifications.close();
      api.dispose();
    });
    await mount(tester, doctor, notifications);
    final initialReads = repo.queueReads;
    repo.current = [];
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pump(const Duration(milliseconds: 249));
    expect(repo.queueReads, initialReads);
    await tester.pump(const Duration(milliseconds: 1751));
    await tester.pumpAndSettle();
    expect(repo.queueReads, initialReads + 1);
    expect(doctor.state.queue, isEmpty);

    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(seconds: 2));
    expect(repo.queueReads, initialReads + 1);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'new backend notification reconciles Queue once; repeated notification state and disposed shell do not refresh',
    (tester) async {
      final repo = LifecycleQueueRepository();
      final doctor = DoctorCubit(repo);
      final api = ApiClient(dio: Dio());
      final notifications = QuietNotifications(NotificationRepository(api));
      addTearDown(() async {
        await tester.pumpWidget(const SizedBox.shrink());
        await doctor.close();
        await notifications.close();
        api.dispose();
      });
      await mount(tester, doctor, notifications);
      final initialReads = repo.queueReads;
      repo.current = [];
      final notice = NotificationDto(
        id: 'committed-message-notice',
        userId: 'doctor-user',
        type: 'NEW_CHAT_MESSAGE',
        title: 'New consultation message',
        body: 'You have a new consultation message.',
        createdAt: DateTime.utc(2026, 10, 16),
        deliveryStatus: 'CREATED',
      );
      notifications.emit(NotificationState(items: [notice]));
      await tester.pumpAndSettle();
      expect(repo.queueReads, initialReads + 1);
      expect(doctor.state.queue, isEmpty);
      notifications.emit(notifications.state.copyWith(unreadCount: 1));
      await tester.pumpAndSettle();
      expect(repo.queueReads, initialReads + 1);
      await tester.pumpWidget(const SizedBox.shrink());
      notifications.emit(const NotificationState());
      await tester.pump(const Duration(seconds: 31));
      expect(repo.queueReads, initialReads + 1);
      expect(tester.takeException(), isNull);
    },
  );
}
