import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/app.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

// Layout tests mount the actual signed-in shell; service loading is separate.
class LayoutDoctorCubit extends DoctorCubit {
  LayoutDoctorCubit(super.repo);
  @override
  Future<void> loadAll() async {}
}

class LayoutNotificationCubit extends NotificationCubit {
  LayoutNotificationCubit(super.repo);
  @override
  Future<void> load() async {}
}

void main() {
  for (final width in [320.0, 360.0, 412.0]) {
    for (final inset in [0.0, 24.0, 34.0, 48.0]) {
      testWidgets('Doctor navigation stays usable at ${width}dp / $inset inset',
          (tester) async {
        tester.view.physicalSize = Size(width, 800);
        tester.view.devicePixelRatio = 1;
        tester.view.padding = FakeViewPadding(bottom: inset);
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        addTearDown(tester.view.resetPadding);
        addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));

        final api = ApiClient(dio: Dio());
        addTearDown(api.dispose);
        await tester.pumpWidget(MultiBlocProvider(providers: [
          BlocProvider<DoctorCubit>(
              create: (_) => LayoutDoctorCubit(DoctorRepository(api))),
          BlocProvider<NotificationCubit>(
              create: (_) =>
                  LayoutNotificationCubit(NotificationRepository(api))),
        ], child: const MaterialApp(home: DoctorShell())));
        await tester.pumpAndSettle();

        final nav = find.byType(DoctorBottomNav);
        expect(nav, findsOneWidget);
        expect(tester.getRect(nav).bottom, 800 - inset);
        for (final label in [
          'Queue',
          'Chat',
          'Scan Rx',
          'Schedule',
          'Wallet',
          'Settings'
        ]) {
          final text = find.descendant(of: nav, matching: find.text(label));
          expect(text, findsOneWidget);
          expect(tester.getRect(text).bottom, lessThanOrEqualTo(800 - inset));
        }
        for (final target in find
            .descendant(of: nav, matching: find.byType(InkWell))
            .evaluate()) {
          expect(tester.getSize(find.byWidget(target.widget)).height,
              greaterThanOrEqualTo(48));
        }

        await tester
            .tap(find.descendant(of: nav, matching: find.text('Schedule')));
        await tester.pumpAndSettle();
        expect(find.byType(SchedulePage), findsOneWidget);
        expect(tester.getRect(nav).bottom, 800 - inset);
        expect(tester.takeException(), isNull);
      });
    }
  }
}
