import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_cubit.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_models.dart';
import 'package:hellodoctor_mobile_patient/features/patient/chat/chat_repository.dart';

/// Chat read receipts and adaptive polling (2026-11-17).
const me = 'patient-user-1';
const doctor = 'doctor-user-1';

ChatMessageDto msg(String id, String sender, String at) =>
    ChatMessageDto.fromJson({
      'id': id,
      'conversation_id': 'conv-1',
      'sender_id': sender,
      'content': 'Message $id',
      'sent_at': at,
    });

class _Store implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

class ReceiptRepo extends ChatRepository {
  ReceiptRepo() : super(ApiClient(dio: Dio(), tokenStore: _Store()));
  List<ChatMessageDto> server = [];
  DateTime? readUpTo;
  final marked = <String>[];
  Completer<void>? holdSend;

  @override
  Future<List<ChatMessageDto>> messages(String conversationId) async =>
      List.of(server);

  @override
  Future<List<ChatMessageDto>> newerMessages(
          String conversationId, ChatMessageDto after) async =>
      server
          .where((m) =>
              m.sentAt.isAfter(after.sentAt) ||
              (m.sentAt.isAtSameMomentAs(after.sentAt) &&
                  m.id.compareTo(after.id) > 0))
          .toList();

  @override
  DateTime? peerReadUpTo(String conversation) => readUpTo;

  @override
  Future<void> markRead(String conversation, String messageId) async {
    marked.add(messageId);
  }

  @override
  Future<ChatMessageDto> sendMessage(
      String conversationId, String content) async {
    await holdSend?.future;
    final sent = msg('mine-${server.length}', me, '2026-11-17T10:05:00Z');
    server.add(sent);
    return sent;
  }
}

void main() {
  test('own messages show Sent, then Read once the doctor has read up to them',
      () async {
    final repo = ReceiptRepo()
      ..server = [
        msg('a', me, '2026-11-17T10:00:00Z'),
        msg('b', me, '2026-11-17T10:01:00Z'),
      ];
    final cubit = ChatCubit(repo, conversationId: 'conv-1', myUserId: me);
    addTearDown(cubit.close);
    await cubit.load();
    final a = cubit.state.messages[0], b = cubit.state.messages[1];
    expect(cubit.state.receiptFor(a), MessageReceipt.sent);

    repo.readUpTo = DateTime.parse('2026-11-17T10:00:00Z');
    await cubit.poll();
    expect(cubit.state.receiptFor(a), MessageReceipt.read);
    expect(cubit.state.receiptFor(b), MessageReceipt.sent);
  });

  test('the newest incoming message is marked read once; own messages never',
      () async {
    final repo = ReceiptRepo()
      ..server = [
        msg('d1', doctor, '2026-11-17T10:00:00Z'),
        msg('p1', me, '2026-11-17T10:01:00Z'),
      ];
    final cubit = ChatCubit(repo, conversationId: 'conv-1', myUserId: me);
    addTearDown(cubit.close);
    await cubit.load();
    await pumpEventQueue();
    expect(repo.marked, ['d1']);

    await cubit.poll(); // nothing new
    await pumpEventQueue();
    expect(repo.marked, ['d1']);

    repo.server.add(msg('d2', doctor, '2026-11-17T10:02:00Z'));
    await cubit.poll();
    await pumpEventQueue();
    expect(repo.marked, ['d1', 'd2']);
  });

  test('a message shows as Sending until the server stores it', () async {
    final repo = ReceiptRepo()..holdSend = Completer<void>();
    final cubit = ChatCubit(repo, conversationId: 'conv-1', myUserId: me);
    addTearDown(cubit.close);
    await cubit.load();
    final sending = cubit.send('Hello doctor');
    await pumpEventQueue();
    expect(cubit.state.pendingText, 'Hello doctor');
    repo.holdSend!.complete();
    await sending;
    expect(cubit.state.pendingText, isNull);
    expect(cubit.state.messages.single.senderId, me);
    expect(cubit.state.receiptFor(cubit.state.messages.single),
        MessageReceipt.sent);
  });

  test('polls every 3 s while active, easing to 10 s then 30 s when quiet',
      () async {
    final repo = ReceiptRepo();
    final cubit = ChatCubit(repo, conversationId: 'conv-1', myUserId: me);
    addTearDown(cubit.close);
    final now = DateTime.now();
    expect(cubit.pollInterval(now), const Duration(seconds: 10),
        reason: 'no activity yet: the previous default');
    await cubit.send('Hi');
    expect(cubit.pollInterval(DateTime.now()), const Duration(seconds: 3));
    expect(cubit.pollInterval(DateTime.now().add(const Duration(minutes: 5))),
        const Duration(seconds: 10));
    expect(cubit.pollInterval(DateTime.now().add(const Duration(minutes: 20))),
        const Duration(seconds: 30));
  });
}
