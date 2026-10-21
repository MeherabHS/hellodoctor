import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_repository.dart';

const _messageJson = {
  'id': 'msg-1',
  'conversation_id': 'conv-1',
  'sender_id': 'doctor-user-1',
  'content': 'Hello, how can I help?',
  'sent_at': '2026-10-10T08:00:00Z',
};

const _secondMessageJson = {
  'id': 'msg-2',
  'conversation_id': 'conv-1',
  'sender_id': 'doctor-user-1',
  'content': 'Hello, how can I help?',
  'sent_at': '2026-10-10T08:00:00Z',
};

/// Stubs only the methods each test needs, per this project's established
/// no-mockito convention (see `family_profiles_cubit_test.dart`'s harness).
class FakeChatRepository extends ChatRepository {
  FakeChatRepository() : super(ApiClient(dio: Dio(), tokenStore: _NoopStore()));

  List<ChatMessageDto>? messagesResult;
  Object? messagesError;
  Object? sendError;
  int sendCallCount = 0;
  String? lastSentContent;

  @override
  Future<List<ChatMessageDto>> messages(String conversationId) async {
    if (messagesError != null) throw messagesError!;
    return messagesResult ?? const [];
  }

  @override
  Future<List<ChatMessageDto>> newerMessages(
          String conversationId, ChatMessageDto after) async =>
      (await messages(conversationId))
          .where((m) =>
              m.sentAt.isAfter(after.sentAt) ||
              (m.sentAt.isAtSameMomentAs(after.sentAt) &&
                  m.id.compareTo(after.id) > 0))
          .toList();

  @override
  Future<ChatMessageDto> sendMessage(
      String conversationId, String content) async {
    sendCallCount++;
    lastSentContent = content;
    if (sendError != null) throw sendError!;
    return ChatMessageDto.fromJson(_secondMessageJson);
  }
}

class _NoopStore implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

void main() {
  late FakeChatRepository repo;
  late ChatCubit cubit;

  setUp(() {
    repo = FakeChatRepository();
    cubit = ChatCubit(repo, conversationId: 'conv-1');
  });

  tearDown(() => cubit.close());

  group('load', () {
    test('populates messages on success', () async {
      repo.messagesResult = [ChatMessageDto.fromJson(_messageJson)];
      await cubit.load();
      expect(cubit.state.messages, hasLength(1));
      expect(cubit.state.error, isNull);
    });

    test('surfaces a repository failure as state.error', () async {
      repo.messagesError = const ApiException(500, 'Server error.');
      await cubit.load();
      expect(cubit.state.error, isNotNull);
      expect(cubit.state.messages, isEmpty);
    });
  });

  group('poll', () {
    test('silently updates messages without flipping loading', () async {
      repo.messagesResult = [ChatMessageDto.fromJson(_messageJson)];
      await cubit.load();
      repo.messagesResult = [
        ChatMessageDto.fromJson(_messageJson),
        ChatMessageDto.fromJson(_secondMessageJson),
      ];
      await cubit.poll();
      expect(cubit.state.messages, hasLength(2));
      expect(cubit.state.loading, isFalse);
    });

    test('a transient failure during poll is silent', () async {
      repo.messagesResult = [ChatMessageDto.fromJson(_messageJson)];
      await cubit.load();
      repo.messagesError = const ApiException(null, 'offline');
      await cubit.poll();
      expect(cubit.state.error, isNull);
      expect(cubit.state.messages, hasLength(1));
    });
  });

  group('send', () {
    test('a no-op for blank content', () async {
      await cubit.send('   ');
      expect(repo.sendCallCount, 0);
    });

    test('sends trimmed content and reloads on success', () async {
      repo.messagesResult = [ChatMessageDto.fromJson(_messageJson)];
      await cubit.send('  Thank you doctor  ');
      expect(repo.sendCallCount, 1);
      expect(repo.lastSentContent, 'Thank you doctor');
      expect(cubit.state.sending, isFalse);
      expect(cubit.state.notSent, isFalse);
      expect(cubit.state.messages, hasLength(1));
    });

    test('a send failure sets notSent without throwing', () async {
      repo.sendError = const ApiException(null, 'offline');
      await cubit.send('Are you there?');
      expect(cubit.state.notSent, isTrue);
      expect(cubit.state.sending, isFalse);
    });

    test('a second send while one is in flight is ignored', () async {
      final first = cubit.send('first message');
      final second = cubit.send('second message');
      await Future.wait([first, second]);
      expect(repo.sendCallCount, 1);
    });
  });
}
