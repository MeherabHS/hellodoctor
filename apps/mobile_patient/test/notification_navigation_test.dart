import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_center_page.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_appointment_page.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_navigation.dart';
import 'package:hellodoctor_mobile_patient/core/notifications/push_notification_service.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';
import 'dart:async';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/notifications/notification_taps.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_destination.dart';
import 'package:hellodoctor_mobile_patient/features/notifications/notification_repository.dart';

const noticeId = '11111111-1111-4111-8111-111111111111';
const entityId = '22222222-2222-4222-8222-222222222222';
const appointmentId = '33333333-3333-4333-8333-333333333333';

class NavigationTokens implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async {
    values[key] = value;
  }

  @override
  Future<void> delete(String key) async {
    values.remove(key);
  }

  @override
  Future<void> clear() async {
    values.clear();
  }
}

class NavigationApi extends ApiClient {
  NavigationApi()
      : super(
            dio: Dio(BaseOptions(baseUrl: 'https://example.test/api/v1')),
            tokenStore: NavigationTokens());
  final requests = <String>[];
  String type = 'BOOKING_CONFIRMED', entityType = 'APPOINTMENT';
  bool reject = false, mismatch = false, denyDestination = false;
  @override
  Future<Response<dynamic>> get(String path,
      {Map<String, dynamic>? query}) async {
    requests.add(path);
    if (reject || denyDestination && !path.startsWith('/notifications/')) {
      throw const ApiException(404, 'Unavailable');
    }
    return Response(requestOptions: RequestOptions(path: path), data: {
      'data': path.startsWith('/notifications/')
          ? {
              'id': noticeId,
              'user_id': 'owner',
              'notification_type': type,
              'title': 'Synthetic',
              'body': 'Synthetic',
              'related_entity_type': entityType,
              'related_entity_id': entityId,
              'metadata': {'appointment_id': appointmentId},
            }
          : {
              'id': mismatch ? appointmentId : entityId,
              'appointment_id': appointmentId,
              'status': 'CONFIRMED',
              'appointment_number': 'SYNTHETIC'
            }
    });
  }
}

class NavigationRepo extends NotificationRepository {
  NavigationRepo(super.api);
  int reads = 0;
  @override
  Future<List<NotificationDto>> list({int? limit, int? offset}) async => [
        NotificationDto.fromJson({
          'id': noticeId,
          'user_id': 'owner',
          'notification_type': 'BOOKING_CONFIRMED',
          'title': 'Appointment update',
          'body': 'Synthetic notice',
          'related_entity_type': 'APPOINTMENT',
          'related_entity_id': entityId
        })
      ];
  @override
  Future<int> unreadCount() async => 1;
  @override
  Future<void> markRead(String id) async {
    reads++;
  }
}

void main() {
  for (final language in ['en', 'sw']) {
    testWidgets('inbox opens its authorized appointment in $language',
        (tester) async {
      final api = NavigationApi(), repo = NavigationRepo(NavigationApi());
      addTearDown(api.dispose);
      addTearDown(repo.api.dispose);
      final cubit = NotificationCubit(repo);
      addTearDown(cubit.close);
      await tester.pumpWidget(BlocProvider.value(
          value: cubit,
          child: MaterialApp(
              locale: Locale(language),
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              home: NotificationCenterPage(cubit: cubit))));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Appointment update'));
      await tester.pumpAndSettle();
      expect(find.byType(NotificationAppointmentPage), findsOneWidget);
      expect(find.text(language == 'sw' ? 'Imethibitishwa' : 'Confirmed'),
          findsOneWidget);
      expect(repo.reads, 1);
    });
  }
  testWidgets(
      'a denied destination stays in the Kiswahili inbox and remains unread',
      (tester) async {
    final api = NavigationApi()..denyDestination = true,
        repo = NavigationRepo(NavigationApi()..denyDestination = true);
    addTearDown(api.dispose);
    addTearDown(repo.api.dispose);
    final cubit = NotificationCubit(repo);
    addTearDown(cubit.close);
    await tester.pumpWidget(BlocProvider.value(
        value: cubit,
        child: MaterialApp(
            locale: const Locale('sw'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: NotificationCenterPage(cubit: cubit))));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Appointment update'));
    await tester.pumpAndSettle();
    expect(find.byType(NotificationAppointmentPage), findsNothing);
    expect(repo.reads, 0);
    expect(cubit.state.unreadCount, 1);
    final l10n = AppLocalizations.of(
        tester.element(find.byType(NotificationCenterPage)));
    expect(find.text(l10n.notificationDestinationError), findsOneWidget);
    expect((repo.api as NavigationApi).requests,
        ['/notifications/$noticeId', '/appointments/$entityId']);
  });
  testWidgets('native tap host handles duplicate callbacks and disposal',
      (tester) async {
    NotificationTaps.clear();
    final api = NavigationApi(), repo = NavigationRepo(NavigationApi());
    addTearDown(api.dispose);
    addTearDown(repo.api.dispose);
    final cubit = NotificationCubit(repo);
    addTearDown(cubit.close);
    await tester.pumpWidget(BlocProvider.value(
        value: cubit,
        child: MaterialApp(
            home: NotificationNavigationHost(
                cubit: cubit, child: const Scaffold(body: Text('Signed in'))),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales)));
    NotificationTaps.receive({'notification_id': noticeId});
    NotificationTaps.receive({'notification_id': noticeId});
    await tester.pumpAndSettle();
    expect(find.byType(NotificationAppointmentPage), findsOneWidget);
    expect(
        (repo.api as NavigationApi)
            .requests
            .where((p) => p == '/notifications/$noticeId')
            .length,
        1);
    await tester.pumpWidget(const SizedBox());
    NotificationTaps.receive({'notification_id': noticeId});
    expect(tester.takeException(), isNull);
    NotificationTaps.clear();
  });
  test(
      'foreground refresh is not an open; explicit taps and disposal are distinct',
      () async {
    NotificationTaps.clear();
    final api = NavigationApi();
    addTearDown(api.dispose);
    var refreshes = 0;
    final service = PushNotificationService(
        repository: NotificationRepository(api),
        onNotificationReceived: () async {
          refreshes++;
        });
    await service.handleForegroundMessage();
    expect(refreshes, 1);
    expect(NotificationTaps.drain(), isEmpty);
    await service.handleOpenedMessage(
        {'notification_id': noticeId, 'url': 'https://evil.test'});
    expect(NotificationTaps.drain(), [noticeId]);
    expect(refreshes, 2);
    await service.dispose();
    await service.handleOpenedMessage({'notification_id': noticeId});
    expect(NotificationTaps.drain(), isEmpty);
    expect(refreshes, 2);
  });
  test('only a well-formed opaque inbox ID crosses the OS boundary', () {
    expect(
        notificationTapId(
            {'notification_id': noticeId, 'url': 'https://evil.test'}),
        noticeId);
    expect(notificationTapId('notification_event:$noticeId'), noticeId);
    for (final invalid in [
      'https://evil.test',
      '../patients/records',
      'javascript:alert(1)',
      '${entityId}x',
      null
    ]) {
      expect(notificationTapId(invalid), isNull);
    }
  });
  test('cold taps wait in memory until an authenticated listener is ready',
      () async {
    NotificationTaps.clear();
    NotificationTaps.receive({'notification_id': noticeId});
    expect(NotificationTaps.drain(), [noticeId]);
    expect(NotificationTaps.drain(), isEmpty);
    final seen = <String?>[];
    final subscription = NotificationTaps.events.listen(seen.add);
    NotificationTaps.receive('notification_event:$noticeId');
    expect(seen, [noticeId]);
    await subscription.cancel();
    NotificationTaps.clear();
  });
  test('duplicate concurrent and completed callbacks open only once', () async {
    final gate = NotificationTapGate(), release = Completer<void>();
    var calls = 0;
    Future<void> open() async {
      calls++;
      await release.future;
    }

    final first = gate.run(noticeId, open);
    await gate.run(noticeId, open);
    expect(calls, 1);
    release.complete();
    await first;
    await gate.run(noticeId, open);
    expect(calls, 1);
    gate.dispose();
  });
  test('a later intentional tap can reopen the same notification', () async {
    var now = DateTime.utc(2026);
    final gate = NotificationTapGate(now: () => now);
    var calls = 0;
    await gate.run(noticeId, () async {
      calls++;
    });
    now = now.add(const Duration(seconds: 3));
    await gate.run(noticeId, () async {
      calls++;
    });
    expect(calls, 2);
    gate.dispose();
  });
  test('failed opens retry and account teardown fences delayed results',
      () async {
    final gate = NotificationTapGate();
    var calls = 0;
    await expectLater(
        gate.run(noticeId, () async {
          calls++;
          throw StateError('Unavailable');
        }),
        throwsStateError);
    await gate.run(noticeId, () async {
      calls++;
    });
    expect(calls, 2);
    final delayed = Completer<void>();
    var navigated = false;
    final pending = gate.run(entityId, () async {
      await delayed.future;
      if (gate.current) navigated = true;
    });
    gate.dispose();
    delayed.complete();
    await pending;
    expect(navigated, isFalse);
  });
  for (final doctor in [false, true]) {
    test(
        'appointment requires canonical notice and fresh authorized read (doctor=$doctor)',
        () async {
      final api = NavigationApi();
      addTearDown(api.dispose);
      final destination = await NotificationDestination.load(
          NotificationRepository(api), noticeId,
          doctor: doctor);
      expect(destination.kind, NotificationDestinationKind.appointment);
      expect(api.requests,
          ['/notifications/$noticeId', '/appointments/$entityId']);
    });
    test('unavailable or unauthorized notice fails closed (doctor=$doctor)',
        () async {
      final api = NavigationApi()..reject = true;
      addTearDown(api.dispose);
      await expectLater(
          NotificationDestination.load(NotificationRepository(api), noticeId,
              doctor: doctor),
          throwsA(isA<ApiException>()));
      expect(api.requests, ['/notifications/$noticeId']);
    });
    test('chat is bound to the authorized exact conversation (doctor=$doctor)',
        () async {
      final api = NavigationApi()
        ..type = 'NEW_CHAT_MESSAGE'
        ..entityType = 'CHAT';
      addTearDown(api.dispose);
      final repo = NotificationRepository(api);
      expect(
          (await NotificationDestination.load(repo, noticeId, doctor: doctor))
              .kind,
          NotificationDestinationKind.chat);
      expect(api.requests.last, '/appointments/$appointmentId/conversation');
      api.mismatch = true;
      await expectLater(
          NotificationDestination.load(repo, noticeId, doctor: doctor),
          throwsFormatException);
    });
    test(
        'unsupported campaign destinations fall back without a clinical read (doctor=$doctor)',
        () async {
      final api = NavigationApi()
        ..type = 'ADMIN_CAMPAIGN'
        ..entityType = 'APPOINTMENT';
      addTearDown(api.dispose);
      expect(
          (await NotificationDestination.load(
                  NotificationRepository(api), noticeId,
                  doctor: doctor))
              .kind,
          NotificationDestinationKind.inbox);
      expect(api.requests, ['/notifications/$noticeId']);
    });
  }
  test('malformed payload cannot issue an arbitrary API request', () async {
    final api = NavigationApi();
    addTearDown(api.dispose);
    expect(
        (await NotificationDestination.load(
                NotificationRepository(api), '../records',
                doctor: false))
            .kind,
        NotificationDestinationKind.inbox);
    expect(api.requests, isEmpty);
  });
  test(
      'published article uses the published API; Doctor editorial uses own API',
      () async {
    final api = NavigationApi()
      ..type = 'NEW_ARTICLE_PUBLISHED'
      ..entityType = 'DOCTOR_ARTICLE';
    addTearDown(api.dispose);
    final repo = NotificationRepository(api);
    expect(
        (await NotificationDestination.load(repo, noticeId, doctor: false))
            .kind,
        NotificationDestinationKind.article);
    expect(api.requests.last, '/articles/$entityId');
    api.type = 'ARTICLE_STATUS_CHANGED';
    expect(
        (await NotificationDestination.load(repo, noticeId, doctor: true)).kind,
        NotificationDestinationKind.doctorArticle);
    expect(api.requests.last, '/doctor/articles/$entityId');
  });
}
