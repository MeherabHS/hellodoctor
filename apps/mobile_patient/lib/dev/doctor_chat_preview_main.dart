// Debug-only design preview for the patient's doctor consultation chat.
//
//   flutter run -t lib/dev/doctor_chat_preview_main.dart
//
// Shows labelled sample messages answered in-process; nothing reaches a
// server. Refuses to run in release.

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../core/network/api_client.dart';
import '../core/theme/tokens.dart';
import '../features/patient/chat/chat_models.dart';
import '../features/patient/chat/chat_repository.dart';
import '../features/patient/booking/booking_models.dart';
import '../features/patient/booking/booking_repository.dart';
import '../features/patient/chat/patient_chat_page.dart';
import '../features/patient/video/agora_call_page.dart';
import '../l10n/app_localizations.dart';
import 'preview_agora_engine.dart';
import 'preview_avatar.dart';

const _language =
    String.fromEnvironment('PREVIEW_LANGUAGE', defaultValue: 'en');

/// `--dart-define=PREVIEW_STATE=EXPIRED` shows the chat after its booked time
/// has ended (read-only, "Expired"). `--dart-define=PREVIEW_SCREEN=CALL`
/// shows a video call (stand-in engine, placeholder video) with the in-call
/// chat button.
const _state = String.fromEnvironment('PREVIEW_STATE', defaultValue: 'ACTIVE');
const _screen = String.fromEnvironment('PREVIEW_SCREEN', defaultValue: 'CHAT');

class _NoTokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

DateTime _ago(int minutes) =>
    DateTime.now().toUtc().subtract(Duration(minutes: minutes));

ChatMessageDto _msg(String id, String sender, String text, int minutesAgo) =>
    ChatMessageDto.fromJson({
      'id': id,
      'conversation_id': 'conv-preview',
      'sender_id': sender,
      'content': text,
      'sent_at': _ago(minutesAgo).toIso8601String(),
    });

class _PreviewChatRepo extends ChatRepository {
  _PreviewChatRepo()
      : super(ApiClient(
            dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1'))
              ..httpClientAdapter = PreviewPhotoAdapter(),
            tokenStore: _NoTokens()));
  final _thread = [
    _msg(
        'm1',
        'doctor',
        'Hello Amina, I have read your intake form. How are you feeling today? (sample)',
        42),
    _msg(
        'm2',
        'me',
        'Hi doctor, the headache is better but I still feel tired in the afternoons. (sample)',
        38),
    _msg(
        'm3',
        'doctor',
        'Thank you. Are you drinking enough water and sleeping at least 7 hours? (sample)',
        35),
    _msg('m4', 'me', 'Mostly yes, about 6 hours on weekdays. (sample)', 31),
  ];

  @override
  Future<List<ChatMessageDto>> messages(String conversationId) async =>
      List.of(_thread);
  @override
  Future<List<ChatMessageDto>> newerMessages(
          String conversation, ChatMessageDto after) async =>
      const [];
  @override
  Future<List<ChatMessageDto>> olderMessages(
          String conversation, ChatMessageDto before) async =>
      const [];
  @override
  Future<ChatMessageDto> sendMessage(
      String conversationId, String content) async {
    final m = _msg('m${_thread.length + 1}', 'me', content, 0);
    _thread.add(m);
    return m;
  }

  @override
  Future<ChatMessageDto> uploadClinicalImage(String conversationId,
          Uint8List bytes, String mimeType, String key) async =>
      throw const ApiException(null, 'Images are not sent in the preview.');
  @override
  Future<void> acknowledge(
      String appointment, String? conversation, String event,
      {List<String> messages = const [],
      DateTime? clientOccurredAt,
      CancelToken? cancelToken}) async {}
}

void main() {
  if (kReleaseMode) {
    runApp(const MaterialApp(
        home: Scaffold(
            body: Center(
                child: Text('The preview is not available in release.')))));
    return;
  }
  runApp(MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      locale: const Locale(_language),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Banner(
          message: 'PREVIEW DATA',
          location: BannerLocation.bottomStart,
          child: _screen == 'CALL'
              ? AgoraCallPage(
                  appointmentId: 'appt-preview',
                  repo: BookingRepository(_PreviewChatRepo().api),
                  engine: PreviewAgoraEngine(),
                  requestPermissions: () async => true,
                  loadToken: () async => const RtcSessionDto(
                      channelName: 'preview',
                      appId: 'preview',
                      token: 'preview',
                      uid: 1,
                      expiresInSeconds: 3600),
                  loadChat: () async => _chat(expired: false))
              : _chat(expired: _state == 'EXPIRED'))));
}

PatientChatPage _chat({required bool expired}) => PatientChatPage(
    conversation: ChatConversationDto(
        id: 'conv-preview',
        appointmentId: 'appt-preview',
        // A chat consultation is open until its booked time ends.
        expiresAt: DateTime.now().toUtc().add(expired
            ? const Duration(minutes: -15)
            : const Duration(minutes: 8)),
        isLocked: false),
    doctorDisplayName: 'Dr. Jane Mwangi (sample)',
    repo: _PreviewChatRepo(),
    myUserId: 'me');
