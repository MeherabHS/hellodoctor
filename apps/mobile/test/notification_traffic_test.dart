import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_cubit.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';

NotificationDto notice(int n) => NotificationDto(
    id: n.toString().padLeft(5, '0'),
    userId: 'owner',
    type: 'NEW_CHAT_MESSAGE',
    title: 'New consultation message',
    body: 'You have a new consultation message.',
    createdAt: DateTime.utc(2026, 10, 16, 0, 0, n),
    deliveryStatus: 'CREATED');

class PageNotifications extends NotificationRepository {
  PageNotifications() : super(ApiClient(dio: Dio()));
  final rows = <NotificationDto>[];
  int lists = 0, deltas = 0, olders = 0, counts = 0;
  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async {
    lists++;
    return rows.reversed.take(25).toList();
  }

  @override
  Future<int> unreadCount() async {
    counts++;
    return rows.length;
  }

  @override
  Future<List<NotificationDto>> newerThan(NotificationDto cursor) async {
    deltas++;
    return rows
        .where((n) =>
            n.createdAt.isAfter(cursor.createdAt) ||
            (n.createdAt == cursor.createdAt && n.id.compareTo(cursor.id) > 0))
        .take(25)
        .toList();
  }

  @override
  Future<List<NotificationDto>> olderThan(NotificationDto cursor) async {
    olders++;
    return rows.reversed
        .where((n) =>
            n.createdAt.isBefore(cursor.createdAt) ||
            (n.createdAt == cursor.createdAt && n.id.compareTo(cursor.id) < 0))
        .take(25)
        .toList();
  }
}

void main() {
  testWidgets(
      'application-wide notification cadence is one delta and count per foreground minute',
      (tester) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    final repo = PageNotifications();
    final cubit = NotificationCubit(repo);
    await cubit.load();
    cubit.startPolling(interval: const Duration(minutes: 1));
    await tester.pump(const Duration(seconds: 59));
    expect(repo.deltas, 0);
    expect(repo.counts, 1);
    await tester.pump(const Duration(seconds: 1));
    await tester.pumpAndSettle();
    expect(repo.deltas, 1);
    expect(repo.counts, 2);
    expect(repo.lists, 1);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.pump(const Duration(minutes: 5));
    expect(repo.deltas, 1);
    expect(repo.counts, 2);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await cubit.close();
    repo.api.dispose();
  });

  testWidgets(
      'quiet notification sync uses deltas, bounds catchup and stops HTTP in background',
      (tester) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    final repo = PageNotifications()..rows.addAll(List.generate(25, notice));
    final cubit = NotificationCubit(repo);
    await cubit.load();
    cubit.startPolling();
    repo.rows.addAll(List.generate(125, (i) => notice(i + 25)));
    await tester.pump(const Duration(seconds: 20));
    await tester.pumpAndSettle();
    expect(repo.lists, 1);
    expect(repo.deltas, 4);
    expect(repo.counts, 2);
    expect(cubit.state.items.length, 125);
    expect(cubit.state.unreadCount, 150);
    await tester.pump(const Duration(seconds: 20));
    await tester.pumpAndSettle();
    expect(cubit.state.items.length, 150);
    expect(cubit.state.items.map((n) => n.id).toSet().length, 150);
    final idleDeltas = repo.deltas;
    await tester.pump(const Duration(seconds: 20));
    await tester.pumpAndSettle();
    expect(repo.deltas, idleDeltas + 1);
    expect(repo.lists, 1);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.paused);
    await tester.pump(const Duration(seconds: 120));
    expect(repo.deltas, idleDeltas + 1);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await cubit.close();
    repo.api.dispose();
  });
  testWidgets(
      'earlier notification pages remain accessible and survive delta reconciliation',
      (tester) async {
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    final repo = PageNotifications()..rows.addAll(List.generate(61, notice));
    final cubit = NotificationCubit(repo);
    await cubit.load();
    expect(cubit.state.items.length, 25);
    expect(cubit.state.hasOlder, isTrue);
    await cubit.loadOlder();
    expect(cubit.state.items.length, 50);
    await cubit.loadOlder();
    expect(cubit.state.items.length, 61);
    expect(cubit.state.hasOlder, isFalse);
    expect(repo.olders, 2);
    repo.rows.add(notice(61));
    cubit.startPolling();
    await tester.pump(const Duration(seconds: 20));
    await tester.pumpAndSettle();
    expect(cubit.state.items.length, 62);
    expect(cubit.state.items.first.id, '00061');
    expect(cubit.state.items.last.id, '00000');
    await cubit.load();
    expect(cubit.state.items.length, 62);
    expect(cubit.state.items.map((n) => n.id).toSet().length, 62);
    await cubit.close();
    repo.api.dispose();
  });
}
