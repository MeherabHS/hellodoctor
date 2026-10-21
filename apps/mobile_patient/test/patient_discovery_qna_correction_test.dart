import 'dart:io';
import 'dart:ui' as ui;
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/core/network/connectivity_service.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/patient_directory_page.dart';
import 'package:hellodoctor_mobile_patient/core/theme/tokens.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/consultation_actions.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/directory_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/directory/doctor_profile_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/profile/family_profiles_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_ask_sheet.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_page.dart';
import 'package:hellodoctor_mobile_patient/features/patient/qna/qna_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';
import 'qna_cubit_test.dart' show FakeQnaRepository;

final doctorJson = <String, dynamic>{
  'id': 'doctor-test',
  'full_name': 'Dr. Alexandra Wanjiku Njeri Mwangi',
  'license_number': 'KMPDC-1234',
  'license_authority': 'KMPDC',
  'license_country': 'KE',
  'primary_specialty': 'Internal Medicine',
  'experience_years': 12,
  'current_hospital':
      'Nairobi Centre for Internal Medicine and Specialist Care',
  'qualifications': ['MBChB'],
  'consultation_fee_video': '1800.00',
  'consultation_fee_chat': '350.00',
  'is_on_duty': true,
};
final openJson = <String, dynamic>{
  'id': 'question-open',
  'question_text': 'When should recurring headaches be assessed?',
  'category': 'General Health',
  'status': 'OPEN',
  'created_at': '2026-10-14T08:00:00Z'
};
final answeredJson = <String, dynamic>{
  ...openJson,
  'id': 'question-answered',
  'status': 'ANSWERED',
  'answer_text':
      'Recurring headaches can have several causes. A clinician can assess the pattern, symptoms and medical history. Seek urgent assessment for a sudden severe headache or new neurological symptoms. This is general guidance and does not replace a consultation.',
  'answered_at': '2026-10-14T09:00:00Z',
  'answering_doctor': 'Dr. Alexandra Wanjiku Njeri Mwangi',
  'answering_doctor_specialty': 'Internal Medicine',
  'answering_doctor_verified': true
};

class MemoryTokens implements TokenStore {
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

Widget host(Widget child, {double scale = 1, String language = 'en'}) =>
    MaterialApp(
      theme: PatientTokens.theme(),
      locale: Locale(language),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      builder: (context, child) => MediaQuery(
          data: MediaQuery.of(context)
              .copyWith(textScaler: TextScaler.linear(scale)),
          child: RepaintBoundary(key: const Key('capture'), child: child!)),
      home: child,
    );
Future<void> viewport(WidgetTester tester, double width) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = Size(width, 900);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

Future<void> capture(WidgetTester tester, String name) async {
  if (!const bool.fromEnvironment('CAPTURE_PATIENT_UI')) return;
  await tester.pump();
  final boundary = tester
      .renderObject<RenderRepaintBoundary>(find.byKey(const Key('capture')));
  await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 2);
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    final file =
        File('../../docs/audit_evidence/patient_ui_correction/$name.png');
    await file.parent.create(recursive: true);
    await file.writeAsBytes(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    final font = FontLoader('Plus Jakarta Sans')
      ..addFont(rootBundle.load('assets/fonts/PlusJakartaSans.ttf'));
    await font.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });

  for (final width in [320.0, 360.0, 390.0, 412.0]) {
    testWidgets('full directory route filters and long profile at ${width}dp',
        (tester) async {
      await viewport(tester, width);
      const method = MethodChannel('dev.fluttercommunity.plus/connectivity');
      const events =
          MethodChannel('dev.fluttercommunity.plus/connectivity_status');
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(method, (call) async => ['wifi']);
      messenger.setMockMethodCallHandler(events, (call) async => null);
      addTearDown(() {
        messenger.setMockMethodCallHandler(method, null);
        messenger.setMockMethodCallHandler(events, null);
      });
      final connectivity = ConnectivityService();
      addTearDown(connectivity.dispose);
      final dio = Dio();
      final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
      addTearDown(api.dispose);
      dio.interceptors.add(InterceptorsWrapper(onRequest: (o, h) {
        final dynamic data = o.path.endsWith('/slots')
            ? <dynamic>[]
            : o.path == '/doctors'
                ? [doctorJson]
                : doctorJson;
        h.resolve(Response(
            requestOptions: o,
            statusCode: 200,
            data: {'success': true, 'data': data}));
      }));
      final family = FamilyProfilesCubit(FamilyProfilesRepository(api));
      addTearDown(family.close);
      await tester.pumpWidget(host(PatientDirectoryPage(
          api: api,
          cache: null,
          connectivity: connectivity,
          familyProfilesCubit: family)));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await capture(tester, 'find_doctor_route_${width.toInt()}');
      await tester.tap(find.text('Availability'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('On duty').last);
      await tester.pumpAndSettle();
      final filters = find.byType(DoctorDirectoryFilters);
      await tester.drag(filters, const Offset(-500, 0));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Visit type'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('24-hour chat').last);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await tester.ensureVisible(find.text('View profile'));
      await tester.tap(find.text('View profile'));
      await tester.pumpAndSettle();
      await capture(tester, 'doctor_profile_route_${width.toInt()}');
      expect(find.text('KMPDC Clinical Status'), findsNothing);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox.shrink());
    });
  }
  testWidgets('Swahili consultation and Q&A at 320dp enlarged text',
      (tester) async {
    await viewport(tester, 320);
    final d = PublicDoctorDto.fromJson(doctorJson);
    await tester.pumpWidget(host(
        Scaffold(
            body: SingleChildScrollView(
                padding: const EdgeInsets.all(20),
                child: PatientDoctorCard(
                    doctor: d, onProfile: () {}, onBook: (_) {}))),
        scale: 1.5,
        language: 'sw'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('Pata ushauri'));
    await tester.tap(find.text('Pata ushauri'));
    await tester.pumpAndSettle();
    expect(find.text('Weka miadi ya video'), findsOneWidget);
    expect(find.text('Weka miadi ya mazungumzo'), findsOneWidget);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(host(
        Scaffold(
            body: QnaFeed(
                state: QnaState(questions: [
                  AnsweredQuestionDto.fromJson(openJson),
                  AnsweredQuestionDto.fromJson(answeredJson)
                ]),
                onAsk: () {},
                onRefresh: () async {})),
        scale: 1.5,
        language: 'sw'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    expect(find.text('Linasubiri jibu'), findsOneWidget);
  });
  for (final width in [320.0, 360.0, 390.0, 412.0]) {
    for (final scale in [1.0, 1.5]) {
      testWidgets('discovery and profile fit ${width}dp text $scale',
          (tester) async {
        await viewport(tester, width);
        final doctor = PublicDoctorDto.fromJson(doctorJson);
        await tester.pumpWidget(host(
            Scaffold(
                body: SingleChildScrollView(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Text('Find a doctor',
                              style: PatientTokens.theme()
                                  .textTheme
                                  .headlineSmall),
                          const SizedBox(height: 16),
                          DoctorDirectoryFilters(
                              specialty: 'Specialty',
                              availability: 'Availability',
                              visitType: 'Visit type',
                              onSpecialty: () {},
                              onAvailability: () {},
                              onVisitType: () {}),
                          const SizedBox(height: 20),
                          PatientDoctorCard(
                              doctor: doctor, onProfile: () {}, onBook: (_) {}),
                        ]))),
            scale: scale));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        final filterText = tester.widget<Text>(find.text('Availability'));
        expect(filterText.maxLines, 1);
        expect(filterText.softWrap, false);
        final scroll = find.byType(SingleChildScrollView).last;
        await tester.drag(scroll, const Offset(-600, 0));
        await tester.ensureVisible(find.text('Visit type'));
        await tester.pumpAndSettle();
        expect(find.text('Visit type').hitTestable(), findsOneWidget);
        await tester.drag(scroll, const Offset(1000, 0));
        await tester.pumpAndSettle();
        if (scale == 1) await capture(tester, 'discovery_${width.toInt()}');
        await tester.pumpWidget(host(
            Scaffold(
                body: SingleChildScrollView(
                    padding: const EdgeInsets.all(24),
                    child: DoctorProfileSummary(doctor: doctor))),
            scale: scale));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        final name = tester.widget<Text>(find.text(doctor.displayName));
        expect(name.maxLines, 2);
        expect(name.textAlign, TextAlign.center);
        if (scale == 1) await capture(tester, 'profile_long_${width.toInt()}');
      });
    }
  }
  for (final modes in [
    ('800', '0', 'Book video', 'VIDEO'),
    ('0', '300', 'Book chat', 'CHAT'),
    ('800', '300', 'Consult', 'BOTH')
  ]) {
    testWidgets('available mode ${modes.$4} routes correct booking',
        (tester) async {
      await viewport(tester, 320);
      final doctor = PublicDoctorDto.fromJson({
        ...doctorJson,
        'consultation_fee_video': modes.$1,
        'consultation_fee_chat': modes.$2
      });
      String? chosen;
      await tester.pumpWidget(host(Scaffold(
          body: SingleChildScrollView(
              padding: const EdgeInsets.all(20),
              child: PatientDoctorCard(
                  doctor: doctor,
                  onProfile: () {},
                  onBook: (m) => chosen = m)))));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      await capture(tester, 'doctor_${modes.$4.toLowerCase()}_320');
      await tester.tap(find.text(modes.$3));
      await tester.pumpAndSettle();
      if (modes.$4 == 'BOTH') {
        expect(find.text('Book video'), findsOneWidget);
        expect(find.text('Book chat'), findsOneWidget);
        await capture(tester, 'consultation_choices_320');
        await tester.tap(find.text('Book chat'));
        await tester.pumpAndSettle();
        expect(chosen, 'CHAT');
      } else {
        expect(chosen, modes.$4);
      }
    });
  }
  test('modality rules reject zero negative and malformed fees', () {
    for (final fee in ['0', '-1', 'NaN', 'bad']) {
      final d = PublicDoctorDto.fromJson(
          {...doctorJson, 'consultation_fee_chat': fee});
      expect(d.supportsChat, false);
    }
  });
  testWidgets('neither mode offers profile without fake booking',
      (tester) async {
    await viewport(tester, 320);
    final d = PublicDoctorDto.fromJson({
      ...doctorJson,
      'consultation_fee_video': '0',
      'consultation_fee_chat': '0'
    });
    await tester.pumpWidget(host(Scaffold(
        body: PatientDoctorCard(doctor: d, onProfile: () {}, onBook: (_) {}))));
    await tester.pumpAndSettle();
    expect(find.text('View profile'), findsOneWidget);
    expect(find.text('Consult'), findsNothing);
    expect(find.text('Book chat'), findsNothing);
    expect(find.text('Book video'), findsNothing);
  });
  testWidgets('specific historic fixture command removed from name',
      (tester) async {
    final d = PublicDoctorDto.fromJson({
      ...doctorJson,
      'full_name': 'Dr. Synthetic Kenya Review 47278ee148 Approve'
    });
    expect(d.fullName, contains('47278ee148'));
    expect(d.displayName, 'Dr. Synthetic Kenya Review');
    await tester
        .pumpWidget(host(Scaffold(body: DoctorProfileSummary(doctor: d))));
    await tester.pumpAndSettle();
    expect(find.textContaining('Approve'), findsNothing);
    expect(find.textContaining('47278ee148'), findsNothing);
  });
  for (final width in [320.0, 360.0, 390.0, 412.0]) {
    testWidgets('editorial Q&A open and answered at ${width}dp',
        (tester) async {
      await viewport(tester, width);
      await tester.pumpWidget(host(Scaffold(
          body: QnaFeed(
              state: QnaState(questions: [
                AnsweredQuestionDto.fromJson(openJson),
                AnsweredQuestionDto.fromJson(answeredJson),
                AnsweredQuestionDto.fromJson({
                  ...answeredJson,
                  'id': 'fixture',
                  'category': 'WEB_FIXTURE',
                  'question_text': 'Synthetic verification fixture'
                })
              ]),
              onAsk: () {},
              onRefresh: () async {}))));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Awaiting answer'), findsOneWidget);
      expect(find.text('Answered by'), findsOneWidget);
      expect(find.textContaining('WEB_FIXTURE'), findsNothing);
      expect(
          find.textContaining('Synthetic verification fixture'), findsNothing);
      expect(find.byType(Card), findsNothing);
      await capture(tester, 'qna_${width.toInt()}');
      await tester.ensureVisible(find.text('Read full answer'));
      await tester.tap(find.text('Read full answer'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(
          tester
              .widget<Text>(
                  find.text(answeredJson['answer_text'] as String).last)
              .maxLines,
          isNull);
    });
  }
  testWidgets('question form validation success OPEN and repeat entry',
      (tester) async {
    await viewport(tester, 320);
    final repo = FakeQnaRepository();
    final cubit = QnaCubit(repo);
    final family = FamilyProfilesCubit(FamilyProfilesRepository(repo.api));
    addTearDown(cubit.close);
    addTearDown(family.close);
    await tester.pumpWidget(host(Scaffold(
        body: Builder(
            builder: (c) => FilledButton(
                onPressed: () => showAskQuestionSheet(c,
                    cubit: cubit, familyProfilesCubit: family),
                child: const Text('Ask'))))));
    await tester.tap(find.text('Ask'));
    await tester.pumpAndSettle();
    await capture(tester, 'question_form_320');
    await tester.ensureVisible(find.byKey(const Key('qna-submit')));
    await tester.tap(find.byKey(const Key('qna-submit')));
    await tester.pumpAndSettle();
    expect(find.text('Enter a question with 1 to 4,000 characters.'),
        findsOneWidget);
    expect(repo.lastSubmittedText, isNull);
    await tester.enterText(find.byKey(const Key('qna-question')),
        'When should recurring headaches be assessed?');
    await tester.ensureVisible(find.byKey(const Key('qna-submit')));
    await tester.tap(find.byKey(const Key('qna-submit')));
    await tester.pumpAndSettle();
    expect(find.text('Question submitted'), findsOneWidget);
    expect(cubit.state.questions.single.status, 'OPEN');
    expect(cubit.state.questions.single.answerText, isEmpty);
    await capture(tester, 'question_submitted_320');
    repo.feedResult = List.of(cubit.state.questions);
    await tester.tap(find.text('Back to questions'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Ask'));
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('qna-question')), findsOneWidget);
    expect(find.text('Question submitted'), findsNothing);
    expect(tester.takeException(), isNull);
  });
  testWidgets('submission failure retains question and permits retry',
      (tester) async {
    await viewport(tester, 360);
    final repo = FakeQnaRepository()
      ..submitError = const ApiException(422, 'Please correct the question.');
    final cubit = QnaCubit(repo);
    final family = FamilyProfilesCubit(FamilyProfilesRepository(repo.api));
    addTearDown(cubit.close);
    addTearDown(family.close);
    await tester.pumpWidget(
        host(AskQuestionPage(cubit: cubit, familyProfilesCubit: family)));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(const Key('qna-question')), 'A general question');
    await tester.ensureVisible(find.byKey(const Key('qna-submit')));
    await tester.tap(find.byKey(const Key('qna-submit')));
    await tester.pumpAndSettle();
    expect(find.text('Please correct the question.'), findsOneWidget);
    expect(cubit.state.questions, isEmpty);
    expect(find.text('Question submitted'), findsNothing);
    repo.submitError = null;
    await tester.tap(find.byKey(const Key('qna-submit')));
    await tester.pumpAndSettle();
    expect(find.text('Question submitted'), findsOneWidget);
  });
  test('real repository sends owned-open query and validates creation response',
      () async {
    final dio = Dio();
    final api = ApiClient(dio: dio, tokenStore: MemoryTokens());
    final repo = QnaRepository(api);
    addTearDown(api.dispose);
    final requests = <RequestOptions>[];
    dio.interceptors.add(InterceptorsWrapper(onRequest: (o, h) {
      requests.add(o);
      h.resolve(Response(
          requestOptions: o,
          statusCode: o.method == 'POST' ? 201 : 200,
          data: {
            'success': true,
            'data': o.method == 'POST'
                ? openJson
                : [
                    openJson,
                    answeredJson,
                    {
                      ...answeredJson,
                      'id': 'fixture',
                      'category': 'WEB_FIXTURE'
                    }
                  ]
          }));
    }));
    final created = await repo.submit(
        questionText: openJson['question_text'] as String,
        category: 'General Health');
    expect(created.status, 'OPEN');
    expect(requests.first.data['question_text'], openJson['question_text']);
    expect(requests.first.data.containsKey('author_patient_id'), false);
    final feed = await repo.feed();
    expect(feed.length, 2);
    expect(requests.last.queryParameters['include_own_open'], true);
    expect(feed.first.isAnswered, false);
    expect(feed.last.isAnswered, true);
  });
  test('empty oversized and duplicate submits never reach repository twice',
      () async {
    final repo = FakeQnaRepository();
    final cubit = QnaCubit(repo);
    addTearDown(cubit.close);
    expect(await cubit.submit(questionText: '  '), false);
    expect(await cubit.submit(questionText: 'a' * 4001), false);
    expect(repo.lastSubmittedText, isNull);
    final first = cubit.submit(questionText: 'Question');
    final duplicate = cubit.submit(questionText: 'Question');
    expect(await duplicate, false);
    expect(await first, true);
    expect(cubit.state.questions.length, 1);
  });
}
