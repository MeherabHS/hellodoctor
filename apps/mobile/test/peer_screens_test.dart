import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile/core/network/api_client.dart';
import 'package:hellodoctor_mobile/features/auth/auth_cubit.dart';
import 'package:hellodoctor_mobile/features/auth/welcome_pages.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_models.dart';
import 'package:hellodoctor_mobile/features/notifications/notification_repository.dart';
import 'package:hellodoctor_mobile/features/peer/peer_pages.dart';
import 'package:hellodoctor_mobile/features/peer/peer_repository.dart';

class _MemoryStore implements TokenStore {
  final values = <String, String>{};
  @override
  Future<String?> read(String key) async => values[key];
  @override
  Future<void> write(String key, String value) async => values[key] = value;
  @override
  Future<void> delete(String key) async => values.remove(key);
  @override
  Future<void> clear() async => values.clear();
}

String _token(String role, String userId) {
  String part(Map<String, Object> m) =>
      base64Url.encode(utf8.encode(jsonEncode(m))).replaceAll('=', '');
  return '${part({'alg': 'EdDSA'})}.${part({
        'role': role,
        'user_id': userId
      })}.sig';
}

ApiClient _api() {
  final store = _MemoryStore()..values['access_token'] = _token('PEER', 'me');
  return ApiClient(dio: Dio(), tokenStore: store);
}

final _soon = DateTime.now().add(const Duration(hours: 2));

PeerInboxItem _item(String id, String status,
        {String name = 'Amina M.', String? chat, String type = 'CHAT'}) =>
    PeerInboxItem(
        id: id,
        patientName: name,
        ageRange: '15-19',
        start: _soon,
        end: _soon.add(const Duration(minutes: 10)),
        supportType: type,
        topic: 'Staying on track',
        status: status,
        conversationId: chat);

class _FakeRepo extends PeerRepository {
  _FakeRepo(
      {this.status = 'APPROVED', List<PeerInboxItem>? items, this.chatList})
      : items = items ??
            [
              _item('r1', 'PENDING'),
              _item('r2', 'ACCEPTED', name: 'Neema K.', chat: 'c2'),
            ],
        super(_api());
  final String status;
  final List<PeerInboxItem> items;
  final List<PeerChatSummary>? chatList;
  final decisions = <String>[];
  final created = <DateTime>[];
  final sent = <String>[];
  final images = <String>[];
  PeerProfile profileValue = const PeerProfile(
      id: 'p1',
      fullName: 'Grace Wanjiku',
      county: 'Nairobi',
      approvalStatus: 'APPROVED');

  @override
  Future<PeerProfile> profile() async => PeerProfile(
      id: 'p1',
      fullName: profileValue.fullName,
      county: profileValue.county,
      bio: profileValue.bio,
      organization: profileValue.organization,
      approvalStatus: status);
  @override
  Future<PeerProfile> updateProfile(
      {String? fullName,
      String? county,
      String? organization,
      String? bio}) async {
    profileValue = PeerProfile(
        id: 'p1',
        fullName: profileValue.fullName,
        county: county ?? profileValue.county,
        organization: organization ?? profileValue.organization,
        bio: bio ?? profileValue.bio,
        approvalStatus: 'APPROVED');
    return profileValue;
  }

  @override
  Future<List<PeerInboxItem>> requests({String? status}) async => items;
  @override
  Future<List<PeerChatSummary>> chats() async =>
      chatList ??
      [
        PeerChatSummary(
            id: 'c2',
            requestId: 'r2',
            name: 'Neema K.',
            supportType: 'CHAT',
            requestStatus: 'ACCEPTED',
            start: _soon,
            unread: 2,
            lastMessage: 'That would be great.',
            lastMessageAt: DateTime.now()),
      ];
  @override
  Future<PeerInboxItem> decide(String id, String action, {String? note}) async {
    decisions.add('$action:$id');
    return _item(id, action == 'accept' ? 'ACCEPTED' : 'COMPLETED', chat: 'c1');
  }

  @override
  Future<List<PeerSlot>> slots(DateTime from, DateTime to) async => [
        PeerSlot(
            id: 's-booked',
            start: from.add(const Duration(hours: 10, minutes: 10)),
            end: from.add(const Duration(hours: 10, minutes: 20)),
            status: 'BOOKED'),
        PeerSlot(
            id: 's-open',
            start: from.add(const Duration(hours: 10)),
            end: from.add(const Duration(hours: 10, minutes: 10)),
            status: 'AVAILABLE'),
      ];
  @override
  Future<PeerSlot> addSlot(DateTime start) async {
    created.add(start);
    return PeerSlot(
        id: 's',
        start: start,
        end: start.add(const Duration(minutes: 10)),
        status: 'AVAILABLE');
  }

  @override
  Future<List<PeerChatMessage>> messages(String chatId) async => [
        PeerChatMessage(
            id: 'm1',
            senderId: 'patient',
            content: 'Hi! Thanks for accepting.',
            sentAt: DateTime.now(),
            hasImage: false),
        PeerChatMessage(
            id: 'm2',
            senderId: 'me',
            content: 'Glad you reached out.',
            sentAt: DateTime.now(),
            hasImage: false,
            isRead: true),
      ];
  @override
  Future<PeerChatMessage> send(String chatId, String content) async {
    sent.add(content);
    return PeerChatMessage(
        id: 'm3',
        senderId: 'me',
        content: content,
        sentAt: DateTime.now(),
        hasImage: false);
  }

  @override
  Future<PeerChatMessage> sendImage(
      String chatId, Uint8List bytes, String mime, String key) async {
    images.add('$mime:${bytes.length}');
    return PeerChatMessage(
        id: 'm4',
        senderId: 'me',
        content: '',
        sentAt: DateTime.now(),
        hasImage: true);
  }

  @override
  Future<void> markRead(String chatId) async {}

  @override
  Future<List<Map<String, dynamic>>> peerDocuments() async => [
        for (final t in [
          'Peer Navigator Terms',
          'Peer Navigator Privacy Notice',
          'Peer Navigator Confidentiality, Conduct & Safeguarding Agreement'
        ])
          {
            'document_type': 'PEER_X',
            'version': '1.0',
            'title': t,
            'body': 'Text'
          }
      ];
}

class _FakeNotifications extends NotificationRepository {
  _FakeNotifications(this.items)
      : super(ApiClient(dio: Dio()), deviceIdStore: _MemoryStore());
  final List<NotificationDto> items;
  final read = <String>[];
  @override
  Future<List<NotificationDto>> list({int? offset, int? limit}) async => items;
  @override
  Future<int> unreadCount() async => items.where((n) => !n.isRead).length;
  @override
  Future<void> markRead(String id) async => read.add(id);
}

NotificationDto _notice(
        String id, String type, String entityType, String entity,
        {Duration ago = const Duration(minutes: 5)}) =>
    NotificationDto(
        id: id,
        userId: 'me',
        type: type,
        title:
            type == 'PEER_REQUEST_NEW' ? 'New support request' : 'New message',
        body: 'Someone has requested a support session with you.',
        createdAt: DateTime.now().subtract(ago),
        deliveryStatus: 'SENT',
        relatedEntityType: entityType,
        relatedEntityId: entity);

Widget _host(Widget child) => BlocProvider(
    create: (_) => AuthCubit(ApiClient(dio: Dio())),
    child: MaterialApp(home: child));

void _phone(WidgetTester tester, {double height = 2400}) {
  tester.view.physicalSize = Size(1080, height);
  tester.view.devicePixelRatio = 2.6;
  addTearDown(tester.view.reset);
}

Widget _app(_FakeRepo repo, {List<NotificationDto> notices = const []}) =>
    _host(PeerApp(repo: repo, notifications: _FakeNotifications(notices)));

void main() {
  test('the signed-in role comes from the access token', () {
    expect(roleFromAccessToken(_token('PEER', 'u1')), 'PEER');
    expect(roleFromAccessToken(_token('DOCTOR', 'u1')), 'DOCTOR');
    expect(roleFromAccessToken('not-a-token'), isNull);
    expect(const SignedIn(role: 'PEER').isPeer, isTrue);
    expect(peerUserIdFromToken(_token('PEER', 'u9')), 'u9');
  });

  testWidgets('a pending peer sees the approval gate, not the tabs',
      (tester) async {
    await tester.pumpWidget(_app(_FakeRepo(status: 'PENDING')));
    await tester.pumpAndSettle();
    expect(find.text('Application under review'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-tab-Requests')), findsNothing);
  });

  testWidgets('home shows the counts, the bell dot and quick actions',
      (tester) async {
    _phone(tester);
    await tester.pumpWidget(_app(_FakeRepo(),
        notices: [_notice('n1', 'PEER_REQUEST_NEW', 'PEER_REQUEST', 'r1')]));
    await tester.pumpAndSettle();
    for (final tab in ['Home', 'Requests', 'Schedule', 'Chats', 'Profile']) {
      expect(find.byKey(ValueKey('peer-tab-$tab')), findsOneWidget);
    }
    expect(find.text('Welcome,\nGrace 👋'), findsOneWidget);
    expect(find.textContaining('Welcome,\nPeer Navigator'), findsNothing);
    expect(find.text('Support. Guide. Connect. Follow Up.'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-bell-dot')), findsOneWidget);
    // 1 pending request, 2 unread chat messages.
    final pendingKpi = find.byKey(const ValueKey('peer-kpi-1'));
    expect(find.descendant(of: pendingKpi, matching: find.text('1')),
        findsOneWidget);
    final unreadKpi = find.byKey(const ValueKey('peer-kpi-3'));
    expect(find.descendant(of: unreadKpi, matching: find.text('2')),
        findsOneWidget);
    await tester.ensureVisible(find.byKey(const ValueKey('peer-quick-slots')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-quick-slots')));
    await tester.pumpAndSettle();
    expect(find.text('Create Your Support Slots'), findsOneWidget);
  });

  testWidgets('requests: pending tab, accept, and open conversation',
      (tester) async {
    _phone(tester);
    final repo = _FakeRepo();
    await tester.pumpWidget(_app(repo));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-tab-Requests')));
    await tester.pumpAndSettle();
    expect(find.text('Support Requests'), findsOneWidget);
    expect(find.text('Amina M.'), findsOneWidget);
    expect(find.text('Neema K.'), findsNothing, reason: 'Pending tab only');
    expect(find.text('Age 15–19'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('accept-r1')));
    await tester.pumpAndSettle();
    expect(repo.decisions, ['accept:r1']);

    await tester.tap(find.byKey(const ValueKey('peer-pill-Accepted')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('open-conversation-r2')), findsOneWidget);
  });

  testWidgets('requests: search filters by name or topic', (tester) async {
    _phone(tester);
    final repo = _FakeRepo(items: [
      _item('r1', 'PENDING'),
      _item('r3', 'PENDING', name: 'Peter K.'),
    ]);
    await tester.pumpWidget(_host(
        Scaffold(body: PeerRequestsTab(repo: repo, onSetAvailability: () {}))));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-requests-search')));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(const ValueKey('peer-requests-query')), 'peter');
    await tester.pumpAndSettle();
    expect(find.text('Peter K.'), findsOneWidget);
    expect(find.text('Amina M.'), findsNothing);
  });

  testWidgets('requests: empty state offers Set Availability', (tester) async {
    _phone(tester);
    var opened = false;
    await tester.pumpWidget(_host(Scaffold(
        body: PeerRequestsTab(
            repo: _FakeRepo(items: []),
            onSetAvailability: () => opened = true))));
    await tester.pumpAndSettle();
    expect(find.text('No support requests yet'), findsOneWidget);
    await tester.tap(find.text('Set Availability'));
    expect(opened, isTrue);
  });

  testWidgets(
      'request details: pending shows accept/decline; accepted can complete',
      (tester) async {
    _phone(tester);
    final repo = _FakeRepo();
    await tester.pumpWidget(_host(
        PeerRequestDetailPage(repo: repo, request: _item('r1', 'PENDING'))));
    await tester.pumpAndSettle();
    expect(find.text('Support Request Details'), findsOneWidget);
    expect(find.text('Waiting'), findsOneWidget);
    expect(find.text('Age Group'), findsOneWidget);
    expect(find.text('About the Request'), findsOneWidget);
    await tester
        .ensureVisible(find.byKey(const ValueKey('peer-detail-accept')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-detail-accept')));
    await tester.pumpAndSettle();
    expect(repo.decisions, ['accept:r1']);
    expect(find.byKey(const ValueKey('peer-detail-call')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-detail-chat')), findsOneWidget);
    // Let the "accepted" snackbar leave before tapping near the bottom.
    await tester.pump(const Duration(seconds: 5));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('mark-complete')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('mark-complete')));
    await tester.pumpAndSettle();
    expect(repo.decisions, ['accept:r1', 'complete:r1']);
  });

  testWidgets('schedule: presets set the range and create 10-minute slots',
      (tester) async {
    _phone(tester, height: 3400);
    final repo = _FakeRepo();
    final now = DateTime.now();
    final early = DateTime(now.year, now.month, now.day, 6);
    await tester.pumpWidget(
        _host(Scaffold(body: PeerScheduleTab(repo: repo, clock: () => early))));
    await tester.pumpAndSettle();
    // 8:00-10:00 by default: 12 slots.
    expect(find.text('Create 12 Slots'), findsOneWidget);
    // The preview strip is gone (product owner, 2026-11-15).
    expect(find.textContaining('Preview ('), findsNothing);
    expect(find.byKey(const ValueKey('peer-preview-8:0')), findsNothing);
    await tester.ensureVisible(find.byKey(const ValueKey('peer-preset-1')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-preset-1')));
    await tester.pumpAndSettle();
    expect(find.text('Create 6 Slots'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('peer-slots-create')));
    await tester.pumpAndSettle();
    expect(repo.created, hasLength(6));
    expect(repo.created.first.hour, 8);
    expect(repo.created.last.minute, 50);
  });

  testWidgets('schedule: an existing slot is skipped and Clear empties it',
      (tester) async {
    _phone(tester, height: 3400);
    final repo = _FakeRepo();
    final now = DateTime.now();
    await tester.pumpWidget(_host(Scaffold(
        body: PeerScheduleTab(
            repo: repo,
            clock: () => DateTime(now.year, now.month, now.day, 6)))));
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.byKey(const ValueKey('peer-preset-3')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-preset-3')));
    await tester.pumpAndSettle();
    // 8:00-11:00 is 18 starts; 10:00 and 10:10 already exist.
    expect(find.text('Create 16 Slots'), findsOneWidget);
    await tester.ensureVisible(find.byKey(const ValueKey('peer-slots-clear')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-slots-clear')));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey('peer-no-slots-hint')), findsOneWidget);
  });

  testWidgets('my schedule lists slots with names and a summary',
      (tester) async {
    _phone(tester, height: 3400);
    final now = DateTime.now();
    final day = DateTime(now.year, now.month, now.day);
    final booked = day.add(const Duration(hours: 10, minutes: 10));
    final repo = _FakeRepo(items: [
      PeerInboxItem(
          id: 'r9',
          patientName: 'Amina M.',
          start: booked,
          end: booked.add(const Duration(minutes: 10)),
          supportType: 'CHAT',
          topic: 't',
          status: 'ACCEPTED'),
    ]);
    await tester.pumpWidget(_host(Scaffold(
        body: PeerScheduleTab(
            repo: repo,
            initialMode: 1,
            clock: () => DateTime(now.year, now.month, now.day, 6)))));
    await tester.pumpAndSettle();
    expect(find.text('Your Schedule'), findsOneWidget);
    expect(find.text('Booked by Amina M.'), findsOneWidget);
    expect(find.text('Open for booking'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-remove-s-open')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-remove-s-booked')), findsNothing);
    expect(find.text('Total Slots'), findsOneWidget);
  });

  testWidgets('chats list shows unread and an empty state', (tester) async {
    _phone(tester);
    await tester.pumpWidget(_host(Scaffold(
        body: PeerChatsTab(repo: _FakeRepo(), onViewRequests: () {}))));
    await tester.pumpAndSettle();
    expect(find.text('Your peer support conversations'), findsOneWidget);
    expect(find.text('Neema K.'), findsOneWidget);
    expect(find.text('That would be great.'), findsOneWidget);

    await tester.pumpWidget(_host(Scaffold(
        body: PeerChatsTab(
            key: UniqueKey(),
            repo: _FakeRepo(chatList: []),
            onViewRequests: () {}))));
    await tester.pumpAndSettle();
    expect(find.text('No conversations yet'), findsOneWidget);
    expect(find.text('View Requests'), findsOneWidget);
  });

  testWidgets('chat: own messages right with ticks; send text and a photo',
      (tester) async {
    _phone(tester);
    final repo = _FakeRepo();
    await tester.pumpWidget(_host(PeerChatPage(
        repo: repo,
        chatId: 'c2',
        requestId: 'r2',
        name: 'Neema K.',
        status: 'ACCEPTED',
        pickImage: (_) async =>
            (Uint8List.fromList([1, 2, 3]), 'image/jpeg'))));
    await tester.pumpAndSettle();
    expect(find.text('Peer support conversation active.'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-chat-call')), findsOneWidget);
    expect(find.byIcon(Icons.done_all_rounded), findsOneWidget);
    Alignment alignOf(String id) => tester
        .widget<Align>(find
            .ancestor(
                of: find.byKey(ValueKey('peer-message-$id')),
                matching: find.byType(Align))
            .last)
        .alignment as Alignment;
    expect(alignOf('m1'), Alignment.centerLeft);
    expect(alignOf('m2'), Alignment.centerRight);

    await tester.enterText(
        find.byKey(const ValueKey('peer-message-input')), 'See you then');
    await tester.tap(find.byKey(const ValueKey('peer-send')));
    await tester.pumpAndSettle();
    expect(repo.sent, ['See you then']);

    await tester.tap(find.byKey(const ValueKey('peer-attach')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-attach-gallery')));
    await tester.pumpAndSettle();
    expect(repo.images, ['image/jpeg:3']);
    expect(find.byKey(const ValueKey('peer-view-image-m4')), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets(
      'notifications: grouped, unread, and a pending request can be viewed',
      (tester) async {
    _phone(tester);
    final repo = _FakeRepo();
    final notes = _FakeNotifications([
      _notice('n1', 'PEER_REQUEST_NEW', 'PEER_REQUEST', 'r1'),
      _notice('n2', 'NEW_CHAT_MESSAGE', 'PEER_CHAT', 'c2',
          ago: const Duration(days: 1)),
    ]);
    await tester.pumpWidget(
        _host(PeerNotificationsPage(repo: repo, notifications: notes)));
    await tester.pumpAndSettle();
    expect(find.text('Notifications'), findsOneWidget);
    expect(find.text('Today'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-notice-unread-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-notice-view-n1')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-notice-decline-n2')), findsNothing);
    await tester.tap(find.byKey(const ValueKey('peer-notice-view-n1')));
    await tester.pumpAndSettle();
    expect(notes.read, ['n1']);
    expect(find.text('Support Request Details'), findsOneWidget);
  });

  testWidgets('notifications: empty state', (tester) async {
    _phone(tester);
    await tester.pumpWidget(_host(PeerNotificationsPage(
        repo: _FakeRepo(), notifications: _FakeNotifications(const []))));
    await tester.pumpAndSettle();
    expect(find.text('No notifications yet'), findsOneWidget);
  });

  testWidgets('profile: edit about, manage slots and sign out are there',
      (tester) async {
    _phone(tester);
    final repo = _FakeRepo();
    var managed = false;
    await tester.pumpWidget(_host(Scaffold(
        body: PeerProfileTab(
            repo: repo,
            profile: await repo.profile(),
            onManageSlots: () => managed = true))));
    await tester.pumpAndSettle();
    expect(find.text('My Profile'), findsOneWidget);
    expect(find.text('Approved'), findsOneWidget);
    expect(find.text('Nairobi'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('peer-profile-edit-about')));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(const ValueKey('peer-profile-edit-field')), 'I listen.');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();
    expect(find.text('I listen.'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('peer-profile-manage-slots')));
    expect(managed, isTrue);
    expect(find.byKey(const ValueKey('peer-sign-out')), findsOneWidget);
  });

  testWidgets('sign-up shows the three peer documents and requires agreement',
      (tester) async {
    _phone(tester, height: 4000);
    await tester.pumpWidget(_host(PeerRegistrationPage(repo: _FakeRepo())));
    await tester.pumpAndSettle();
    expect(find.text('Peer Navigator Terms'), findsOneWidget);
    expect(find.text('Peer Navigator Privacy Notice'), findsOneWidget);
    expect(
        find.text(
            'Peer Navigator Confidentiality, Conduct & Safeguarding Agreement'),
        findsOneWidget);
  });
}
