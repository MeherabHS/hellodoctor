import 'dart:io';
import 'dart:ui' show ImageByteFormat;
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/core/theme/tokens.dart';
import 'package:hellodoctor_mobile/features/doctor/clinical_message_bubble.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_cubit.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_models.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_pages.dart';
import 'package:hellodoctor_mobile/features/doctor/doctor_repository.dart';
import 'doctor_widgets_test.dart' show repoHarness;

class ImagePickerFixture extends FilePicker {
  ImagePickerFixture({this.fileSize = 4});
  final int fileSize;
  int cleared = 0;
  @override
  Future<FilePickerResult?> pickFiles(
          {String? dialogTitle,
          String? initialDirectory,
          FileType type = FileType.any,
          List<String>? allowedExtensions,
          Function(FilePickerStatus)? onFileLoading,
          bool allowCompression = true,
          int compressionQuality = 30,
          bool allowMultiple = false,
          bool withData = false,
          bool withReadStream = false,
          bool lockParentWindow = false,
          bool readSequential = false}) async =>
      FilePickerResult([
        PlatformFile(
            name: 'clinical.jpg',
            size: fileSize,
            bytes: Uint8List.fromList([255, 216, 255, 0]))
      ]);
  @override
  Future<bool?> clearTemporaryFiles() async {
    cleared++;
    return true;
  }
}

class HistoryFixtureRepository extends DoctorRepository {
  HistoryFixtureRepository(this.completed) : super(ApiClient(dio: Dio()));
  final AppointmentDto completed;
  @override
  Future<List<AppointmentDto>> consultationHistory({int offset = 0}) async =>
      offset == 0 ? [completed] : [];
}

class FailedImageRepository extends DoctorRepository {
  FailedImageRepository() : super(ApiClient(dio: Dio()));
  final keys = <String>[];
  @override
  Future<List<MessageDto>> messages(String id) async => [];
  @override
  Future<MessageDto> uploadClinicalImage(
      String id, Uint8List bytes, String mime, String key) async {
    keys.add(key);
    throw const ApiException(409, 'Storage unavailable');
  }
}

Future<void> capture(WidgetTester tester, String name) async {
  if (!const bool.fromEnvironment('CAPTURE_PRE_E2E')) {
    return;
  }
  final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(const ValueKey('capture-pre-e2e')));
  await tester.runAsync(() async {
    final image = await boundary.toImage(pixelRatio: 2);
    final bytes = await image.toByteData(format: ImageByteFormat.png);
    await File(
            '../../docs/audit_evidence/existing_features_device_e2e/$name.png')
        .writeAsBytes(bytes!.buffer.asUint8List());
    image.dispose();
  });
}

class VisualChatRepository extends FailedImageRepository {
  @override
  Future<List<MessageDto>> messages(String id) async => const [
        MessageDto(
            id: 'p',
            content: 'Hello doctor. This is a consultation test.',
            sentAt: '2026-10-16T00:00:00Z',
            senderId: 'patient-user'),
        MessageDto(
            id: 'd',
            content: 'Hello, I received your message.',
            sentAt: '2026-10-16T00:01:00Z',
            senderId: 'doctor-user'),
      ];
}

Widget productChat() => BlocProvider<DoctorCubit>(
    create: (_) {
      final cubit = DoctorCubit(VisualChatRepository());
      cubit.emit(const DoctorState(
          profile: ProfileDto(
              name: 'Dr. Test',
              license: 'Test',
              hospital: 'Test Clinic',
              specialty: 'General Medicine',
              verificationStatus: 'VERIFIED',
              qualifications: [],
              userId: 'doctor-user')));
      return cubit;
    },
    child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: DoctorTokens.theme(),
        home: ChatThreadPage(
            chat: ChatDto(
                id: 'c',
                patientName: 'Patient Test Account',
                expiresAt: DateTime.now()
                    .toUtc()
                    .add(const Duration(hours: 24))
                    .toIso8601String(),
                locked: false,
                modality: 'CHAT'))));

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
  testWidgets(
      'default queue uses server eligibility and preserves completed history',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(360, 2400));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final now = DateTime.now().toUtc();
    AppointmentDto item(
            String id, String status, bool actionable, int minutes) =>
        AppointmentDto.fromJson({
          'id': id,
          'patient_display_name': id,
          'status': status,
          'modality': 'CHAT',
          'start_time': now.add(Duration(minutes: minutes)).toIso8601String(),
          'queue_actionable': actionable
        });
    await tester.pumpWidget(repoHarness(
        const QueuePage(),
        DoctorState(queue: [
          item('Current', 'IN_CONSULTATION', true, -60),
          item('Nearest', 'CONFIRMED', true, 30),
          item('Later', 'CONFIRMED', true, 120),
          item('Tomorrow', 'CONFIRMED', false, 1440),
          item('Completed patient', 'COMPLETED', false, -120),
          item('Old unpaid', 'PENDING_PAYMENT', false, -180),
          item('Stale confirmed', 'CONFIRMED', false, -240)
        ]),
        HistoryFixtureRepository(
            item('Completed patient', 'COMPLETED', false, -120))));
    expect(find.text('Completed patient'), findsNothing);
    expect(find.text('Stale confirmed'), findsNothing);
    expect(find.text('Old unpaid'), findsNothing);
    expect(tester.getTopLeft(find.text('Current')).dy,
        lessThan(tester.getTopLeft(find.text('Nearest')).dy));
    expect(tester.getTopLeft(find.text('Nearest')).dy,
        lessThan(tester.getTopLeft(find.text('Later')).dy));
    expect(find.text('Later'), findsOneWidget);
    expect(find.text('Tomorrow'), findsNothing);
    expect(find.textContaining('Completed ('), findsNothing);
    await tester.ensureVisible(find.text('Consultation History'));
    await tester.tap(find.text('Consultation History'));
    await tester.pumpAndSettle();
    expect(find.text('Completed patient'), findsOneWidget);
    expect(find.text('Nearest'), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });
  test('legacy cached records without server eligibility cannot enter upcoming',
      () {
    expect(
        AppointmentDto.fromJson(
                {'status': 'CONFIRMED', 'start_time': '2099-01-01T00:00:00Z'})
            .isQueueActionable,
        isFalse);
    expect(
        AppointmentDto.fromJson(
                {'status': 'COMPLETED', 'queue_actionable': true})
            .isQueueActionable,
        isFalse);
  });
  for (final mine in [true, false]) {
    testWidgets(
        'sender identity controls ${mine ? 'own green right' : 'incoming white left'} on history reload',
        (tester) async {
      for (var load = 0; load < 2; load++) {
        await tester.pumpWidget(MaterialApp(
            home: Scaffold(
                body: ClinicalMessageBubble(
                    message: MessageDto.fromJson({
                      'id': 'm',
                      'sender_id': mine ? 'doctor-user' : 'patient-user',
                      'content':
                          'A long clinical message that wraps clearly across a narrow phone without clipping.',
                      'sent_at': '2026-10-16T00:00:00Z'
                    }),
                    myUserId: 'doctor-user'))));
        final container =
            tester.widget<Container>(find.byKey(const ValueKey('message-m')));
        expect((container.decoration as BoxDecoration).color,
            mine ? DoctorTokens.primary : DoctorTokens.surface);
        final align = tester.widget<Align>(find
            .ancestor(
                of: find.byKey(const ValueKey('message-m')),
                matching: find.byType(Align))
            .first);
        expect(align.alignment,
            mine ? Alignment.centerRight : Alignment.centerLeft);
        expect(find.text('03:00'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.pumpWidget(const SizedBox.shrink());
      }
    });
  }
  testWidgets(
      'paperclip opens clinical attachment flow; failed uploads have no false sent message and retry same key',
      (tester) async {
    FilePickerIO.registerWith();
    final original = FilePicker.platform;
    FilePicker.platform = ImagePickerFixture();
    addTearDown(() => FilePicker.platform = original);
    final repo = FailedImageRepository();
    await tester.pumpWidget(repoHarness(
        const ChatThreadPage(
            chat: ChatDto(
                id: 'c',
                patientName: 'Patient Test Account',
                expiresAt: '2099-01-01T00:00:00Z',
                locked: false,
                modality: 'CHAT')),
        const DoctorState(),
        repo));
    await tester.pumpAndSettle();
    final clip = find.byTooltip('Attach clinical image');
    expect(clip, findsOneWidget);
    expect(tester.getSize(clip).width, greaterThanOrEqualTo(48));
    expect(tester.getSize(clip).height, greaterThanOrEqualTo(48));
    await tester.tap(clip);
    await tester.pumpAndSettle();
    expect(find.text('Attach to consultation'), findsOneWidget);
    await tester.tap(find.text('Clinical image'));
    await tester.pumpAndSettle();
    expect(find.byType(ClinicalMessageBubble), findsNothing);
    expect(
        find.text(
            'Document upload is currently unavailable. Your image was not sent.'),
        findsOneWidget);
    expect(repo.keys, hasLength(1));
    await tester.tap(find.text('Retry image upload'));
    await tester.pumpAndSettle();
    expect(repo.keys, hasLength(2));
    expect(repo.keys[0], repo.keys[1]);
    expect(find.byType(ClinicalMessageBubble), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets(
      'oversized selected image clears picker temporary files without sending',
      (tester) async {
    FilePickerIO.registerWith();
    final original = FilePicker.platform;
    final picker = ImagePickerFixture(fileSize: 11 * 1024 * 1024);
    FilePicker.platform = picker;
    addTearDown(() => FilePicker.platform = original);
    final repo = FailedImageRepository();
    await tester.pumpWidget(repoHarness(
        const ChatThreadPage(
            chat: ChatDto(
                id: 'c',
                patientName: 'Patient Test Account',
                expiresAt: '2099-01-01T00:00:00Z',
                locked: false,
                modality: 'CHAT')),
        const DoctorState(),
        repo));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Attach clinical image'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Clinical image'));
    await tester.pumpAndSettle();
    expect(picker.cleared, 1);
    expect(repo.keys, isEmpty);
    expect(find.byType(ClinicalMessageBubble), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });
  testWidgets(
      'narrow chat composer stays above keyboard and navigation; empty send is disabled',
      (tester) async {
    tester.view.physicalSize = const Size(320, 850);
    tester.view.devicePixelRatio = 1;
    tester.view.padding = const FakeViewPadding(bottom: 24);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPadding);
    addTearDown(tester.view.resetViewInsets);
    await tester.pumpWidget(RepaintBoundary(
        key: const ValueKey('capture-pre-e2e'), child: productChat()));
    await tester.pumpAndSettle();
    final send = find.byTooltip('Send message');
    expect(tester.widget<IconButton>(find.byType(IconButton).last).onPressed,
        isNull);
    expect(tester.getBottomRight(send).dy, lessThanOrEqualTo(826));
    await capture(tester, 'correction_doctor_chat_composer_320');
    await tester.enterText(find.byType(TextField),
        'A test message across multiple lines.\nSecond line.');
    tester.view.viewInsets = const FakeViewPadding(bottom: 300);
    await tester.pump();
    expect(tester.widget<IconButton>(find.byType(IconButton).last).onPressed,
        isNotNull);
    expect(tester.getBottomRight(send).dy, lessThanOrEqualTo(550));
    expect(tester.getSize(send).height, greaterThanOrEqualTo(48));
    expect(tester.takeException(), isNull);
    await capture(tester, 'correction_doctor_chat_keyboard_320');
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
