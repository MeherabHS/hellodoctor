import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';

const profile = ProfileDto(
    name: 'Synthetic Doctor',
    license: 'SYNTH-KE-REGISTRATION-1234567890',
    hospital: 'Synthetic Clinic',
    specialty: 'General',
    verificationStatus: 'VERIFIED',
    qualifications: []);
const question = QnaDto(
    id: 'q-long',
    question:
        'This is a long community question. It needs its full text visible, with no fixed card height or horizontal clipping. How should I seek appropriate care? This paragraph repeats to exercise small phone constraints and an expanded answer editor.',
    status: 'OPEN',
    category: 'General health and wellbeing');

Widget harness(DoctorState state,
        {double textScale = 1,
        DoctorCubit? cubit,
        Widget child = const QnaPage()}) =>
    BlocProvider.value(
        value: cubit ??
            (DoctorCubit(DoctorRepository(ApiClient(dio: Dio())))..emit(state)),
        child: MaterialApp(
            theme: DoctorTokens.theme(),
            builder: (context, child) => MediaQuery(
                data: MediaQuery.of(context)
                    .copyWith(textScaler: TextScaler.linear(textScale)),
                child: child!),
            home: child));

class QnaRepository extends DoctorRepository {
  QnaRepository() : super(ApiClient(dio: Dio()));
  bool conflict = false, unavailable = false;
  List<QnaDto> rows = [question];
  @override
  Future<List<QnaDto>> questions({int limit = 25, int offset = 0}) async {
    if (unavailable) {
      throw const ApiException(503, 'Internal Redis error secret');
    }
    return rows;
  }

  @override
  Future<Json> answer(String id, String answerText) async {
    rows = [
      const QnaDto(
          id: 'q-long',
          question: 'An authoritative answered question',
          status: 'ANSWERED',
          answer: 'Authoritative community answer',
          answeringDoctor: 'Synthetic Doctor B')
    ];
    if (conflict) throw const ApiException(409, 'Already answered');
    return {};
  }
}

void main() {
  // Render with the app's already-bundled font and Flutter's existing icons.
  setUpAll(() async {
    final text = FontLoader('Plus Jakarta Sans')
      ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf'));
    await text.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });
  for (final width in [360.0, 390.0, 412.0, 480.0]) {
    testWidgets('Q&A long registration and accessible text at $width',
        (tester) async {
      await tester.binding.setSurfaceSize(Size(width, 800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(harness(
          const DoctorState(profile: profile, questions: [question]),
          textScale: 2));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.scrollUntilVisible(find.text(question.question), 200,
          scrollable: find.byType(Scrollable).first);
      expect(find.text(question.question), findsOneWidget);
      await tester.scrollUntilVisible(find.byType(FilledButton), 200,
          scrollable: find.byType(Scrollable).first);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });
  }

  for (final width in [360.0, 390.0, 412.0]) {
    testWidgets(
        'Q&A actual modal editor remains reachable above keyboard at $width',
        (tester) async {
      tester.view.physicalSize = Size(width, 844);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final repo = QnaRepository();
      final cubit = DoctorCubit(repo)
        ..emit(const DoctorState(profile: profile, questions: [question]));
      addTearDown(cubit.close);
      await tester.pumpWidget(harness(cubit.state,
          cubit: cubit,
          child: Builder(
              builder: (context) => Scaffold(
                  body: TextButton(
                      onPressed: () => showDoctorQna(context),
                      child: const Text('Open Q&A'))))));
      await tester.tap(find.text('Open Q&A'));
      await tester.pumpAndSettle();
      expect(find.text('Open Health Q&A Triage'), findsOneWidget);
      await tester.scrollUntilVisible(find.byType(TextField), 180,
          scrollable: find.byType(Scrollable).first);
      await tester.enterText(
          find.byType(TextField), 'General community health information');
      tester.view.viewInsets = const FakeViewPadding(bottom: 300);
      await tester.pumpAndSettle();
      await tester.scrollUntilVisible(find.byType(FilledButton), 100,
          scrollable: find.byType(Scrollable).first);
      await tester.ensureVisible(find.byType(FilledButton));
      await tester.pumpAndSettle();
      expect(tester.getBottomRight(find.byType(FilledButton)).dy,
          lessThanOrEqualTo(544));
      expect(tester.takeException(), isNull);
      if (width == 390) {
        await expectLater(find.byType(QnaPage),
            matchesGoldenFile('goldens/qna-editor-keyboard.png'));
      }
      await tester.tap(find.byType(FilledButton));
      await tester.pumpAndSettle();
      expect(find.byType(TextField), findsNothing);
    });
  }

  testWidgets('409 refreshes only Q&A and renders authoritative answer',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = QnaRepository()..conflict = true;
    final cubit = DoctorCubit(repo)
      ..emit(const DoctorState(profile: profile, questions: [question]));
    addTearDown(cubit.close);
    await tester.pumpWidget(harness(cubit.state, cubit: cubit));
    await tester.scrollUntilVisible(find.byType(TextField), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.enterText(find.byType(TextField), 'Community advice');
    await tester.ensureVisible(find.byType(FilledButton));
    await tester.tap(find.byType(FilledButton));
    await tester.pumpAndSettle();
    expect(
        find.text('This question has already been answered.'), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    await tester.scrollUntilVisible(find.text('Answered'), -200,
        scrollable: find.byType(Scrollable).first);
    await tester.tap(find.text('Answered'));
    await tester.pumpAndSettle();
    expect(find.text('Authoritative community answer'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('successful publication with failed refresh never reopens editor',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = QnaRepository()..unavailable = true;
    final cubit = DoctorCubit(repo)
      ..emit(const DoctorState(profile: profile, questions: [question]));
    addTearDown(cubit.close);
    await tester.pumpWidget(harness(cubit.state, cubit: cubit));
    await tester.scrollUntilVisible(find.byType(TextField), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.enterText(find.byType(TextField), 'Community information');
    await tester.ensureVisible(find.byType(FilledButton));
    await tester.tap(find.byType(FilledButton));
    await tester.pumpAndSettle();
    expect(find.text('Answer published.'), findsOneWidget);
    expect(find.byType(TextField), findsNothing);
    expect(find.byType(FilledButton), findsNothing);
    expect(find.text('Refresh answer'), findsOneWidget);
    expect(find.textContaining('Internal Redis'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  final states = <String, DoctorState>{
    'unanswered': const DoctorState(profile: profile, questions: [
      QnaDto(
          id: 'q-short',
          question: 'How can I keep a healthy daily routine?',
          status: 'OPEN',
          category: 'General')
    ]),
    'long-question': const DoctorState(profile: profile, questions: [question]),
    'answered': const DoctorState(profile: profile, questions: [
      QnaDto(
          id: 'q-a',
          question: 'How can I stay well?',
          status: 'ANSWERED',
          answer: 'General health information from a synthetic doctor.',
          answeringDoctor: 'Synthetic Doctor')
    ]),
    'loading': const DoctorState(profile: profile, loading: true),
    'empty': const DoctorState(profile: profile),
    'error': const DoctorState(
        profile: profile, error: 'Internal SQL exception secret'),
    'offline-cache': const DoctorState(
        profile: profile, offline: true, questions: [question]),
  };
  testWidgets(
      '409 with failed refresh closes stale editor and offers safe retry',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final repo = QnaRepository()
      ..conflict = true
      ..unavailable = true;
    final cubit = DoctorCubit(repo)
      ..emit(const DoctorState(profile: profile, questions: [question]));
    addTearDown(cubit.close);
    await tester.pumpWidget(harness(cubit.state, cubit: cubit));
    await tester.scrollUntilVisible(find.byType(TextField), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.enterText(find.byType(TextField), 'Community information');
    await tester.ensureVisible(find.byType(FilledButton));
    await tester.tap(find.byType(FilledButton));
    await tester.pumpAndSettle();
    expect(find.byType(TextField), findsNothing);
    expect(find.byType(FilledButton), findsNothing);
    expect(find.text('Refresh answer'), findsOneWidget);
    expect(find.textContaining('Internal Redis'), findsNothing);
    expect(tester.takeException(), isNull);
  });
  for (final entry in states.entries) {
    testWidgets('Q&A ${entry.key} rendered state', (tester) async {
      await tester.binding.setSurfaceSize(const Size(390, 844));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      await tester.pumpWidget(harness(entry.value,
          child: const RepaintBoundary(
              key: ValueKey('qna-render'), child: QnaPage())));
      await tester.pump(const Duration(milliseconds: 200));
      if (entry.key == 'answered') {
        await tester.tap(find.text('Answered'));
        await tester.pump();
        expect(
            find.textContaining('General health information'), findsOneWidget);
      }
      expect(find.textContaining('Internal SQL'), findsNothing);
      expect(tester.takeException(), isNull);
      await expectLater(find.byKey(const ValueKey('qna-render')),
          matchesGoldenFile('goldens/qna-${entry.key}.png'));
    });
  }
}
