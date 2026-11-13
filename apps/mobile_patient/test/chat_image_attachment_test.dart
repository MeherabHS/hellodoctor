import 'dart:typed_data';

import 'package:dio/dio.dart' show CancelToken;
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/patient_chat_page.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

import 'chat_cubit_test.dart' show FakeChatRepository;

/// Product owner, 2026-11-13: patients attach images in the consultation
/// chat (paperclip), and a failed send can be retried without posting twice.
class ImageChatRepository extends FakeChatRepository {
  final keys = <String>[];
  Object? uploadError;

  @override
  Future<ChatMessageDto> uploadClinicalImage(String conversationId,
      Uint8List bytes, String mimeType, String key) async {
    keys.add(key);
    if (uploadError != null) throw uploadError!;
    return ChatMessageDto.fromJson({
      'id': 'img-1',
      'conversation_id': conversationId,
      'sender_id': 'patient-user',
      'content': '',
      'sent_at': '2026-11-13T08:00:00Z',
      'attachment_id': 'img-1',
      'attachment': {'id': 'img-1', 'size_bytes': 2048},
    });
  }

  @override
  Future<void> acknowledge(
      String appointment, String? conversation, String event,
      {List<String> messages = const [],
      DateTime? clientOccurredAt,
      CancelToken? cancelToken}) async {}
}

final _bytes = Uint8List.fromList([0xff, 0xd8, 0xff, 0xe0]);

void main() {
  test('a sent image appears in the thread', () async {
    final repo = ImageChatRepository();
    final cubit = ChatCubit(repo, conversationId: 'conv-1');
    addTearDown(cubit.close);
    await cubit.sendImage(_bytes, 'image/jpeg');
    expect(cubit.state.uploadingImage, isFalse);
    expect(cubit.state.imageError, isNull);
    expect(cubit.state.messages.single.attachmentId, 'img-1');
  });

  test('a failed image is kept and retried with the same request key',
      () async {
    final repo = ImageChatRepository()
      ..uploadError = const ApiException(null, 'offline');
    final cubit = ChatCubit(repo, conversationId: 'conv-1');
    addTearDown(cubit.close);
    await cubit.sendImage(_bytes, 'image/jpeg');
    expect(cubit.state.imagePending, isTrue);
    expect(cubit.state.imageError, isNotNull);
    expect(cubit.state.messages, isEmpty);

    repo.uploadError = null;
    await cubit.retryImage();
    expect(repo.keys, hasLength(2));
    expect(repo.keys.toSet(), hasLength(1),
        reason: 'a retry must reuse the key so it cannot post twice');
    expect(cubit.state.imagePending, isFalse);
    expect(cubit.state.messages.single.attachmentId, 'img-1');
  });

  test('a closed consultation (403) locks the chat and drops the image',
      () async {
    final repo = ImageChatRepository()
      ..uploadError =
          const ApiException(403, 'This consultation is read-only.');
    final cubit = ChatCubit(repo, conversationId: 'conv-1');
    addTearDown(cubit.close);
    await cubit.sendImage(_bytes, 'image/png');
    expect(cubit.state.serverLocked, isTrue);
    expect(cubit.state.imagePending, isFalse);
  });

  testWidgets(
      'the chat composer shows a paperclip that offers camera and '
      'gallery', (tester) async {
    final repo = ImageChatRepository();
    await tester.pumpWidget(MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: PatientChatPage(
            conversation: ChatConversationDto(
                id: 'conv-1',
                appointmentId: 'apt-1',
                expiresAt:
                    DateTime.now().toUtc().add(const Duration(hours: 20)),
                isLocked: false),
            doctorDisplayName: 'Dr. Sample',
            repo: repo,
            myUserId: 'patient-user')));
    await tester.pump(const Duration(milliseconds: 300));

    final clip = find.byIcon(Icons.attach_file);
    expect(clip, findsOneWidget);
    expect(find.byTooltip('Attach image'), findsOneWidget);
    await tester.tap(clip);
    await tester.pumpAndSettle();
    expect(find.text('Attach to consultation'), findsOneWidget);
    expect(find.text('Take a photo'), findsOneWidget);
    expect(find.text('Choose from gallery'), findsOneWidget);
    await tester.pumpWidget(const SizedBox.shrink());
  });

  testWidgets('a closed chat has no paperclip', (tester) async {
    await tester.pumpWidget(MaterialApp(
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        home: PatientChatPage(
            conversation: ChatConversationDto(
                id: 'conv-1',
                appointmentId: 'apt-1',
                expiresAt:
                    DateTime.now().toUtc().subtract(const Duration(hours: 1)),
                isLocked: true),
            doctorDisplayName: 'Dr. Sample',
            repo: ImageChatRepository(),
            myUserId: 'patient-user')));
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byIcon(Icons.attach_file), findsNothing);
    await tester.pumpWidget(const SizedBox.shrink());
  });
}
