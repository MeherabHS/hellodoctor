import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/time/kenya_time.dart';
import 'package:hellodoctor_mobile/core/widgets/doctor_widgets.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

// QueuePage/SchedulePage/WalletPage/ChatListPage/ScannerPage all read a
// NotificationCubit for the shared header bell (see
// `doctor_pages.dart`'s `_notificationBellActions`), so every harness here
// must provide one alongside DoctorCubit. It is never `.load()`-ed in these
// tests — nothing here exercises the bell's own data, just the screens that
// now sit beside it — so it stays in its default, request-free state.
List<BlocProvider> _notificationProvider() => [
      BlocProvider<NotificationCubit>(
          create: (_) =>
              NotificationCubit(NotificationRepository(ApiClient(dio: Dio()))))
    ];

Widget harness(Widget child, DoctorState state) =>
    MultiBlocProvider(providers: [
      BlocProvider<DoctorCubit>(create: (_) {
        final cubit = DoctorCubit(DoctorRepository(ApiClient(dio: Dio())));
        cubit.emit(state);
        return cubit;
      }),
      ..._notificationProvider(),
    ], child: MaterialApp(home: child));

Widget repoHarness(Widget child, DoctorState state, DoctorRepository repo) =>
    MultiBlocProvider(providers: [
      BlocProvider<DoctorCubit>(create: (_) {
        final cubit = DoctorCubit(repo);
        cubit.emit(state);
        return cubit;
      }),
      ..._notificationProvider(),
    ], child: MaterialApp(home: child));

/// A minimal live repository stub for Schedule block/unblock widget tests:
/// serves [loadAll]'s dependent reads from fixed, empty/neutral data and
/// records calls to [updateSlotStatus], mutating the served slot list so the
/// subsequent `loadAll()` reload (triggered the same way `cancelSlot()`
/// already does) reflects the new status.
class _StubScheduleRepository extends DoctorRepository {
  _StubScheduleRepository(this._slots) : super(ApiClient(dio: Dio()));
  List<SlotDto> _slots;
  final List<String> updateCalls = [];
  @override
  Future<CachedDoctorSnapshot> cachedSnapshot() async =>
      const CachedDoctorSnapshot();
  @override
  Future<ProfileDto> profile() async => const ProfileDto(
      name: 'Dr. Test Doctor',
      license: '1',
      hospital: 'Test Hospital',
      specialty: 'General',
      verificationStatus: 'VERIFIED',
      qualifications: []);
  @override
  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async =>
      [];
  @override
  Future<List<SlotDto>> slots(
          {int limit = 25, int offset = 0, DateTime? day}) async =>
      offset == 0 ? _slots : [];
  @override
  Future<List<ChatDto>> chats({int limit = 25, int offset = 0}) async => [];
  @override
  Future<List<QnaDto>> questions({int limit = 25, int offset = 0}) async => [];
  @override
  Future<WalletDto> wallet() async =>
      const WalletDto(gross: '0', net: '0', pending: '0');
  @override
  Future<Json> duty() async => {'is_on_duty': false};
  @override
  Future<Json> updateSlotStatus(String id, String status) async {
    updateCalls.add('$id:$status');
    _slots = _slots
        .map((s) => s.id == id
            ? SlotDto(
                id: s.id,
                status: status,
                start: s.start,
                end: s.end,
                appointmentId: s.appointmentId,
                patientId: s.patientId,
                patientName: s.patientName)
            : s)
        .toList();
    return {'slot_id': id, 'status': status};
  }
}

void main() {
  testWidgets('Doctor home compose shortcut opens the existing article editor',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(harness(const QueuePage(), const DoctorState()));
    expect(find.byTooltip('Write an article'), findsOneWidget);
    await tester.tap(find.byTooltip('Write an article'));
    await tester.pumpAndSettle();
    expect(
        find.descendant(
            of: find.byType(AppBar), matching: find.text('Article')),
        findsOneWidget);
    expect(find.byType(TextFormField), findsAtLeastNWidgets(3));
  });
  testWidgets('status chip and card render prototype primitives',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(
        const MaterialApp(home: DoctorCard(child: StatusChip('UPCOMING'))));
    expect(find.text('UPCOMING'), findsOneWidget);
  });

  testWidgets('queue renders empty state without fake patient data',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(harness(const QueuePage(), const DoctorState()));
    expect(find.text('No scheduled consultations'), findsOneWidget);
    // Regression guard: the profile header used to hardcode a demo name
    // ("Dr. Sabrina Akter") instead of the real signed-in doctor. With no
    // profile loaded yet it must show a neutral loading state, never a
    // fabricated identity.
    expect(find.text('Dr. Sabrina Akter'), findsNothing);
    expect(find.text('Loading profile…'), findsOneWidget);
  });

  testWidgets('queue profile header shows the real signed-in doctor',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    const profile = ProfileDto(
        name: 'Dr. Server Doctor',
        license: '99999',
        hospital: 'Test Hospital',
        specialty: 'General',
        verificationStatus: 'VERIFIED',
        qualifications: []);
    await tester.pumpWidget(
        harness(const QueuePage(), const DoctorState(profile: profile)));
    expect(find.text('Dr. Server Doctor'), findsOneWidget);
    expect(find.text('✓  KMPDC Registration: 99999'), findsOneWidget);
  });

  testWidgets('queue renders server appointment data', (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final appointment = AppointmentDto.fromJson({
      'id': 'a',
      'patient_id': 'p',
      'patient_display_name': 'Server patient',
      'status': 'CONFIRMED',
      'queue_actionable': true,
      'doctor_lifecycle': 'CONFIRMED'
    });
    await tester.pumpWidget(
        harness(const QueuePage(), DoctorState(queue: [appointment])));
    expect(find.text('Consultation History'), findsOneWidget);
    await tester.pump();
    expect(find.text('Server patient'), findsOneWidget);
    expect(find.text('Scheduled'), findsOneWidget);
  });

  testWidgets('queue shows safe retry state for server failure',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(harness(
        const QueuePage(), const DoctorState(error: 'Service unavailable')));
    expect(find.text('Service unavailable'), findsOneWidget);
    expect(find.text('Retry'), findsOneWidget);
  });

  testWidgets('queue action toolbar stays on one line at common Android widths',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    // A long name/complaint plus the narrowest realistic widths is exactly
    // the combination that previously forced "Call Patient" to wrap onto
    // three lines, breaking "Patient" mid-word.
    final appointment = AppointmentDto.fromJson({
      'id': 'a',
      'patient_id': 'p',
      'patient_display_name': 'Mohammad Abdur Rahman Chowdhury',
      'status': 'CONFIRMED',
      'queue_actionable': true,
      'modality': 'VIDEO',
      'conversation_id': 'shared-conversation',
      'doctor_lifecycle': 'UPCOMING',
      'chief_complaint':
          'Severe abdominal pain with recurring fever since yesterday',
    });
    for (final width in [360.0, 390.0, 412.0]) {
      await tester.binding.setSurfaceSize(Size(width, 800));
      await tester.pumpWidget(
          harness(const QueuePage(), DoctorState(queue: [appointment])));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: 'width=$width');
      expect(find.text('Consultation History'), findsOneWidget);
      await tester.pump();
      expect(find.text('Start video consultation'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Follow-up messages'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Write prescription'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Start chat consultation'), findsNothing,
          reason: 'width=$width');
    }
    await tester.binding.setSurfaceSize(null);
  });

  testWidgets(
      'schedule shows human-readable time and real patient linkage, no overflow',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final tomorrow = DateTime.now().add(const Duration(days: 1));
    final bookedStart =
        DateTime(tomorrow.year, tomorrow.month, tomorrow.day, 10, 30);
    final booked = SlotDto(
        id: 's-booked',
        status: 'BOOKED',
        start: bookedStart,
        end: bookedStart.add(const Duration(minutes: 10)),
        appointmentId: 'a-booked',
        patientId: 'p1',
        patientName: 'Mohammad Abdur Rahman Chowdhury');
    final available = SlotDto(
        id: 's-open',
        status: 'AVAILABLE',
        start: bookedStart.add(const Duration(hours: 1)),
        end: bookedStart.add(const Duration(hours: 1, minutes: 10)));
    final appointment = AppointmentDto.fromJson({
      'id': 'a-booked',
      'patient_id': 'p1',
      'patient_display_name': 'Mohammad Abdur Rahman Chowdhury',
      'status': 'CONFIRMED',
      'modality': 'VIDEO',
      'doctor_lifecycle': 'BOOKED',
    });

    for (final width in [360.0, 390.0, 412.0]) {
      await tester.binding.setSurfaceSize(Size(width, 800));
      await tester.pumpWidget(harness(const SchedulePage(),
          DoctorState(slots: [booked, available], queue: [appointment])));
      await tester.pumpAndSettle();
      // Jump the date slider to the slots' date (they're seeded "tomorrow"
      // so this always exercises the non-today card, not just day one).
      await tester.tap(find.text('${bookedStart.day}').first);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: 'width=$width');
      // Must never show a raw ISO timestamp.
      expect(find.textContaining('T00:00:00'), findsNothing,
          reason: 'width=$width');
      expect(find.textContaining('Mohammad Abdur Rahman Chowdhury'),
          findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Start video consultation'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Start chat consultation'), findsNothing,
          reason: 'width=$width');
      expect(find.text('Cancel Slot ✕'), findsOneWidget,
          reason: 'width=$width');
    }
    await tester.binding.setSurfaceSize(null);
  });

  testWidgets('blocked Web slots do not become mobile patient bookings',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final now = kenyaTime(DateTime.now());
    final start = kenyaWallTime(now.year, now.month, now.day, 23, 0);
    final blocked = SlotDto.fromJson({
      'id': 'blocked-web-slot',
      'status': 'BLOCKED',
      'start_time': start.toIso8601String(),
      'end_time': start.add(const Duration(minutes: 10)).toIso8601String()
    });
    expect(blocked.status, 'BLOCKED');
    await tester.pumpWidget(
        harness(const SchedulePage(), DoctorState(slots: [blocked])));
    await tester.pumpAndSettle();
    expect(find.text('Blocked'), findsWidgets);
    expect(
        find.text('Blocked ? unavailable for patient booking'), findsOneWidget);
    expect(find.text('Booked (0)'), findsOneWidget);
    expect(find.text('Available (0)'), findsOneWidget);
    expect(find.text('Call Patient'), findsNothing);
    expect(find.textContaining('Cancel Slot'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets(
      'booked slot cannot be self-cancelled; open slot cancel requires confirmation',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final tomorrow = DateTime.now().add(const Duration(days: 1));
    final bookedStart =
        DateTime(tomorrow.year, tomorrow.month, tomorrow.day, 10, 30);
    final booked = SlotDto(
        id: 's-booked',
        status: 'BOOKED',
        start: bookedStart,
        end: bookedStart.add(const Duration(minutes: 10)),
        appointmentId: 'a-booked',
        patientId: 'p1',
        patientName: 'Mohammad Abdur Rahman Chowdhury');
    final available = SlotDto(
        id: 's-open',
        status: 'AVAILABLE',
        start: bookedStart.add(const Duration(hours: 1)),
        end: bookedStart.add(const Duration(hours: 1, minutes: 10)));
    final appointment = AppointmentDto.fromJson({
      'id': 'a-booked',
      'patient_id': 'p1',
      'patient_display_name': 'Mohammad Abdur Rahman Chowdhury',
      'status': 'BOOKED',
      'doctor_lifecycle': 'BOOKED',
    });

    await tester.pumpWidget(harness(const SchedulePage(),
        DoctorState(slots: [booked, available], queue: [appointment])));
    await tester.pumpAndSettle();
    await tester.tap(find.text('${bookedStart.day}').first);
    await tester.pumpAndSettle();

    // The booked slot must never expose a self-cancel affordance — the
    // backend's cancel_own_slot only ever permits cancelling AVAILABLE
    // slots — it shows the "cannot be self-cancelled" note instead.
    expect(find.textContaining('cannot be self-cancelled'), findsOneWidget);

    // Exactly one "Cancel Slot ✕" button exists (the open slot's), and
    // tapping it must require confirmation before it can take effect.
    expect(find.text('Cancel Slot ✕'), findsOneWidget);
    await tester.ensureVisible(find.text('Cancel Slot ✕'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancel Slot ✕'));
    await tester.pumpAndSettle();
    expect(find.text('Cancel this slot?'), findsOneWidget);

    // Choosing "Keep slot" must dismiss the dialog without cancelling —
    // the slot and its button must still be there afterward.
    await tester.tap(find.text('Keep slot'));
    await tester.pumpAndSettle();
    expect(find.text('Cancel this slot?'), findsNothing);
    expect(find.text('Cancel Slot ✕'), findsOneWidget);
  });

  testWidgets(
      'wallet shows real settlement breakdown, business-rule wording, no overflow',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    const wallet = WalletDto(
        gross: '35562.50',
        withheld: '7112.50',
        net: '28450.00',
        pending: '0',
        platformChargePercent: '20');

    for (final width in [360.0, 390.0, 412.0]) {
      await tester.binding.setSurfaceSize(Size(width, 900));
      // Pumping EarningsBreakdownCard directly (not WalletPage) — WalletPage
      // also kicks off a real repo.ledger() network call in initState that
      // this plain MaterialApp harness can't serve and would otherwise hang.
      // The breakdown card is the actual business-rule-critical unit here
      // and renders entirely off its own `wallet` input.
      await tester.pumpWidget(const MaterialApp(
          home: Scaffold(
              body: SingleChildScrollView(
                  child: EarningsBreakdownCard(
                      wallet: wallet, consultCount: 4)))));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: 'width=$width');
      expect(find.text('Settlement Breakdown'), findsOneWidget,
          reason: 'width=$width');
      expect(
          find.textContaining(
              'Total calculation is based including the platform charge 20%'),
          findsOneWidget,
          reason: 'width=$width');
      // Forbidden business-rule-violating wording must never appear.
      expect(find.textContaining('Transfer to bKash'), findsNothing,
          reason: 'width=$width');
      expect(find.textContaining('Escrow Cleared'), findsNothing,
          reason: 'width=$width');
      expect(find.textContaining('withdrawal'), findsNothing,
          reason: 'width=$width');
      expect(find.text('Scheduled by Finance'), findsOneWidget,
          reason: 'width=$width');
    }
    await tester.binding.setSurfaceSize(null);
  });

  testWidgets(
      'payout method card shows real configured state, not a placeholder',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: PayoutMethodCard(
                payoutMethod: null, loading: false, onTap: () {}))));
    expect(find.textContaining('Payout Setup Required'), findsOneWidget);

    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body: PayoutMethodCard(payoutMethod: const {
      'configured': true,
      'display_label': 'M-Pesa •••• 5678',
    }, loading: false, onTap: () {}))));
    expect(find.textContaining('M-Pesa •••• 5678'), findsOneWidget);
    expect(find.textContaining('Payout Setup Required'), findsNothing);
  });

  test(
      'ledger payment_status maps honestly — PAYMENT_HELD is never shown as settled/escrow',
      () {
    // PAYMENT_HELD is a real, non-final transaction state (CLAUDE.md:
    // "PAYMENT_HELD is NOT legal escrow"). Getting this wrong means telling a
    // doctor money is theirs before it actually is.
    final (heldLabel, _, _) = settlementBadge('PAYMENT_HELD');
    expect(heldLabel, 'Held');
    expect(heldLabel.toLowerCase(), isNot(contains('settled')));
    expect(heldLabel.toLowerCase(), isNot(contains('escrow')));

    final (settledLabel, _, _) = settlementBadge('SETTLED_TO_DOCTOR');
    expect(settledLabel, 'Settled ✓');
    final (failedLabel, _, _) = settlementBadge('FAILED');
    expect(failedLabel, 'Failed');
  });

  testWidgets('Q&A shows real unanswered questions, quick presets, no overflow',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final question = QnaDto.fromJson({
      'id': 'q1',
      'question_text':
          'My 6-year-old has had a fever of 101°F for two days with mild cough, what should I do?',
      'status': 'OPEN',
      'category': 'Pediatrics',
      'created_at': DateTime.now()
          .toUtc()
          .subtract(const Duration(hours: 2))
          .toIso8601String(),
    });
    const profile = ProfileDto(
        name: 'Dr. Test Doctor',
        license: '45821',
        hospital: 'Test Hospital',
        specialty: 'General',
        verificationStatus: 'VERIFIED',
        qualifications: []);

    for (final width in [360.0, 390.0, 412.0]) {
      await tester.binding.setSurfaceSize(Size(width, 900));
      await tester.pumpWidget(harness(const QnaPage(),
          DoctorState(questions: [question], profile: profile)));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull, reason: 'width=$width');
      expect(find.text('Open Health Q&A Triage'), findsOneWidget,
          reason: 'width=$width');
      expect(find.textContaining('KMPDC #45821'), findsOneWidget,
          reason: 'width=$width');
      expect(find.textContaining('fever of 101'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('2h ago'), findsNothing, reason: 'width=$width');
      expect(find.textContaining('2h ago'), findsOneWidget,
          reason: 'width=$width');
      expect(find.text('Publish Verified Doctor Answer'), findsOneWidget,
          reason: 'width=$width');
      // Tap a preset and confirm it actually lands in the answer field.
      await tester.enterText(find.byType(TextField), '');
      await tester.tap(find.text('Hydration & ORS'));
      await tester.pump();
      expect(find.textContaining('Oral hydration with ORS'), findsOneWidget,
          reason: 'width=$width');
    }
    await tester.binding.setSurfaceSize(null);
  });

  testWidgets('cached content exposes refresh and offline state',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(MaterialApp(
        home: CacheStatusBar(
            offline: true,
            refreshing: true,
            lastUpdated: DateTime(2026, 10, 5, 8, 42))));
    expect(find.textContaining('Offline'), findsOneWidget);
    expect(find.textContaining('Last updated'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
  });

  testWidgets(
      'schedule gates Block/Unblock exactly like Cancel: AVAILABLE shows '
      'Block, BOOKED shows neither', (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final tomorrow = DateTime.now().add(const Duration(days: 1));
    final bookedStart =
        DateTime(tomorrow.year, tomorrow.month, tomorrow.day, 10, 30);
    final booked = SlotDto(
        id: 's-booked',
        status: 'BOOKED',
        start: bookedStart,
        end: bookedStart.add(const Duration(minutes: 10)),
        appointmentId: 'a-booked',
        patientId: 'p1',
        patientName: 'Booked Patient');
    final available = SlotDto(
        id: 's-open',
        status: 'AVAILABLE',
        start: bookedStart.add(const Duration(hours: 1)),
        end: bookedStart.add(const Duration(hours: 1, minutes: 10)));
    final appointment = AppointmentDto.fromJson({
      'id': 'a-booked',
      'patient_id': 'p1',
      'patient_display_name': 'Booked Patient',
      'status': 'BOOKED',
      'doctor_lifecycle': 'BOOKED',
    });

    await tester.pumpWidget(harness(const SchedulePage(),
        DoctorState(slots: [booked, available], queue: [appointment])));
    await tester.pumpAndSettle();
    await tester.tap(find.text('${bookedStart.day}').first);
    await tester.pumpAndSettle();

    // The AVAILABLE slot shows both Cancel and Block; the BOOKED slot shows
    // neither (same gating as the existing self-cancel restriction).
    expect(find.text('Cancel Slot ✕'), findsOneWidget);
    expect(find.text('Block'), findsOneWidget);
    expect(find.text('Unblock Slot'), findsNothing);
  });

  testWidgets('BLOCKED slot shows only Unblock, never Cancel or Block',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final now = kenyaTime(DateTime.now());
    final start = kenyaWallTime(now.year, now.month, now.day, 23, 0);
    final blocked = SlotDto(
        id: 's-blocked',
        status: 'BLOCKED',
        start: start,
        end: start.add(const Duration(minutes: 10)));
    await tester.pumpWidget(
        harness(const SchedulePage(), DoctorState(slots: [blocked])));
    await tester.pumpAndSettle();
    expect(find.text('Unblock Slot'), findsOneWidget);
    expect(find.textContaining('Cancel Slot'), findsNothing);
    expect(find.text('Block'), findsNothing);
  });

  testWidgets(
      'tapping Block on an available slot calls the repository and updates '
      'the displayed status', (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    final now = kenyaTime(DateTime.now());
    final start = kenyaWallTime(now.year, now.month, now.day, 23, 0);
    final available = SlotDto(
        id: 's-open',
        status: 'AVAILABLE',
        start: start,
        end: start.add(const Duration(minutes: 10)));
    final repo = _StubScheduleRepository([available]);

    await tester.pumpWidget(repoHarness(
        const SchedulePage(), DoctorState(slots: [available]), repo));
    await tester.pumpAndSettle();

    expect(find.text('Block'), findsOneWidget);
    expect(find.text('Unblock Slot'), findsNothing);
    await tester.tap(find.text('Block'));
    await tester.pumpAndSettle();

    expect(repo.updateCalls, ['s-open:BLOCKED']);
    expect(find.text('Unblock Slot'), findsOneWidget);
    expect(find.text('Block'), findsNothing);
  });

  testWidgets(
      'deferred financial module notice replaces earnings UI with no fabricated data',
      (tester) async {
    addTearDown(() => tester.pumpWidget(const SizedBox.shrink()));
    await tester.pumpWidget(const MaterialApp(home: DeferredWalletNotice()));
    await tester.pumpAndSettle();

    expect(find.text('Financial module not yet active'), findsOneWidget);
    expect(find.text('Payment collection is not yet active for this launch.'),
        findsOneWidget);
    expect(find.textContaining('Consultations are proceeding without charge'),
        findsOneWidget);
    // No earnings/ledger/payout UI must leak through this branch.
    expect(find.text('Settlement Breakdown'), findsNothing);
    expect(find.textContaining('KES'), findsNothing);
  });
}
