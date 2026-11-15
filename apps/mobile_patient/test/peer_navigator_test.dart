import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/core/l10n/kenya_time.dart';
import 'package:hellodoctor_mobile_patient/core/network/api_client.dart';
import 'package:hellodoctor_mobile_patient/features/patient/booking/booking_repository.dart';
import 'package:hellodoctor_mobile_patient/features/patient/modules/patient_modules.dart';
import 'package:hellodoctor_mobile_patient/features/patient/peer/peer_pages.dart';
import 'package:hellodoctor_mobile_patient/features/patient/peer/peer_repository.dart';
import 'package:hellodoctor_mobile_patient/l10n/app_localizations.dart';

class _Tokens implements TokenStore {
  @override
  Future<String?> read(String key) async => null;
  @override
  Future<void> write(String key, String value) async {}
  @override
  Future<void> delete(String key) async {}
  @override
  Future<void> clear() async {}
}

ApiClient _api() => ApiClient(
    dio: Dio(BaseOptions(baseUrl: 'http://127.0.0.1:1/api/v1')),
    tokenStore: _Tokens());

/// 10:00 EAT tomorrow in Kenya (the screens group days in Kenya time).
final _tomorrow10 = () {
  final k = kenyaTime(DateTime.now());
  return DateTime.utc(k.year, k.month, k.day + 1, 7).toLocal();
}();

const _peer =
    PeerNavigatorDto(id: 'p1', fullName: 'Neema K.', county: 'Nairobi');

PatientPeerRequestDto _req(String id, String status, {String? chat}) =>
    PatientPeerRequestDto(
        id: id,
        peerId: 'p1',
        peerName: 'Neema K.',
        start: _tomorrow10,
        end: _tomorrow10.add(const Duration(minutes: 10)),
        supportType: 'CHAT',
        topic: 'Staying on track',
        status: status,
        conversationId: chat);

class _Repo extends PatientPeerRepository {
  _Repo({this.peerList = const [_peer], this.requests = const []})
      : super(_api());
  final List<PeerNavigatorDto> peerList;
  List<PatientPeerRequestDto> requests;
  final created = <Map<String, String>>[];
  final cancelled = <String>[];
  final sent = <String>[];

  @override
  Future<List<PeerNavigatorDto>> peers() async => peerList;
  @override
  Future<List<PeerOpenSlotDto>> openSlots(String peerId) async => [
        PeerOpenSlotDto(
            id: 's1',
            start: _tomorrow10,
            end: _tomorrow10.add(const Duration(minutes: 10))),
        PeerOpenSlotDto(
            id: 's2',
            start: _tomorrow10.add(const Duration(hours: 1)),
            end: _tomorrow10.add(const Duration(hours: 1, minutes: 10))),
      ];
  @override
  Future<PatientPeerRequestDto> request(
      {required String slotId,
      required String supportType,
      required String topic}) async {
    created.add({'slot': slotId, 'type': supportType, 'topic': topic});
    return _req('r-new', 'PENDING');
  }

  @override
  Future<List<PatientPeerRequestDto>> myRequests() async => requests;
  @override
  Future<PatientPeerRequestDto> cancel(String requestId) async {
    cancelled.add(requestId);
    requests = [];
    return _req(requestId, 'CANCELLED');
  }

  @override
  Future<List<PeerChatMessageDto>> messages(String chatId) async => [
        PeerChatMessageDto(
            id: 'm1',
            senderId: 'peer-user',
            content: 'Hi Amina, I am glad you reached out.',
            sentAt: DateTime.now(),
            hasImage: false),
        PeerChatMessageDto(
            id: 'm2',
            senderId: 'me',
            content: 'Thank you!',
            sentAt: DateTime.now(),
            hasImage: false,
            isRead: true),
      ];
  @override
  Future<PeerChatMessageDto> send(String chatId, String content) async {
    sent.add(content);
    return PeerChatMessageDto(
        id: 'm3',
        senderId: 'me',
        content: content,
        sentAt: DateTime.now(),
        hasImage: false);
  }

  final images = <(String, int)>[];
  @override
  Future<PeerChatMessageDto> sendImage(
      String chatId, Uint8List bytes, String mime, String key) async {
    images.add((mime, bytes.length));
    return PeerChatMessageDto(
        id: 'm4',
        senderId: 'me',
        content: '',
        sentAt: DateTime.now(),
        hasImage: true);
  }

  @override
  Future<void> markRead(String chatId) async {}
}

const _nav = PatientShellNav(
    openServices: _noop, openMessages: _noop, openProfile: _noop, select: _sel);
void _noop() {}
void _sel(int _) {}

Widget _host(Widget child, {String language = 'en'}) => MaterialApp(
    locale: Locale(language),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: child);

void _tall(WidgetTester tester) {
  tester.view.physicalSize = const Size(1080, 3200);
  tester.view.devicePixelRatio = 2.6;
  addTearDown(tester.view.reset);
}

void main() {
  testWidgets('booking needs a time and a topic, then sends the request',
      (tester) async {
    _tall(tester);
    final repo = _Repo();
    await tester.pumpWidget(_host(BookPeerSupportPage(
        repo: repo,
        bookingRepo: BookingRepository(_api()),
        nav: _nav,
        peer: _peer)));
    await tester.pumpAndSettle();
    expect(find.text('Book Peer Support'), findsOneWidget);
    expect(find.text('Neema K.'), findsOneWidget);
    expect(find.textContaining('Tomorrow'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('peer-request-support')));
    await tester.pumpAndSettle();
    expect(find.text('Choose a time.'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey('slot-s1')));
    await tester.tap(find.byKey(const ValueKey('support-CALL')));
    await tester.enterText(find.byKey(const ValueKey('peer-topic')), 'hi');
    await tester.tap(find.byKey(const ValueKey('peer-request-support')));
    await tester.pumpAndSettle();
    expect(find.text('Write at least 3 characters about your topic.'),
        findsOneWidget);
    expect(repo.created, isEmpty);

    await tester.enterText(
        find.byKey(const ValueKey('peer-topic')), 'Sticking to my PrEP');
    await tester.tap(find.byKey(const ValueKey('peer-request-support')));
    await tester.pumpAndSettle();
    expect(repo.created, [
      {'slot': 's1', 'type': 'CALL', 'topic': 'Sticking to my PrEP'}
    ]);
    expect(find.text('Support Request Sent!'), findsOneWidget);
    expect(find.text('Waiting for peer acceptance'), findsOneWidget);
    expect(find.textContaining('as soon as Neema K. accepts'), findsOneWidget);
  });

  testWidgets('the topic is capped at 200 characters', (tester) async {
    _tall(tester);
    await tester.pumpWidget(_host(BookPeerSupportPage(
        repo: _Repo(),
        bookingRepo: BookingRepository(_api()),
        nav: _nav,
        peer: _peer)));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(const ValueKey('peer-topic')), 'x' * 250);
    await tester.pump();
    final field =
        tester.widget<TextField>(find.byKey(const ValueKey('peer-topic')));
    expect(field.controller!.text.length, peerTopicMaxLength);
  });

  testWidgets('the list shows approved peers and an honest empty state',
      (tester) async {
    await tester.pumpWidget(_host(PeerNavigatorListPage(
        repo: _Repo(peerList: const []),
        bookingRepo: BookingRepository(_api()),
        nav: _nav)));
    await tester.pumpAndSettle();
    expect(find.text('No Peer Navigators available yet'), findsOneWidget);

    await tester.pumpWidget(_host(PeerNavigatorListPage(
        key: UniqueKey(),
        repo: _Repo(),
        bookingRepo: BookingRepository(_api()),
        nav: _nav)));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-p1')));
    await tester.pumpAndSettle();
    expect(find.text('Book Peer Support'), findsOneWidget);
  });

  testWidgets('a pending request can be cancelled; an accepted one opens chat',
      (tester) async {
    _tall(tester);
    final repo = _Repo(requests: [
      _req('r1', 'PENDING'),
      _req('r2', 'ACCEPTED', chat: 'c2'),
    ]);
    await tester.pumpWidget(_host(Scaffold(
        body: ListView(children: [
      PatientPeerRequestsSection(
          repo: repo, bookingRepo: BookingRepository(_api()), myUserId: 'me'),
    ]))));
    await tester.pumpAndSettle();
    expect(find.text('My support requests'), findsOneWidget);
    expect(find.text('Waiting for acceptance'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-open-chat-r2')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-cancel-r2')), findsNothing);

    await tester.tap(find.byKey(const ValueKey('peer-cancel-r1')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Cancel request').last);
    await tester.pumpAndSettle();
    expect(repo.cancelled, ['r1']);
    expect(find.text('My support requests'), findsNothing);
  });

  testWidgets(
      'peer chat matches the reference: header, banner, day chip, ticks, '
      'sending appends', (tester) async {
    final repo = _Repo();
    await tester.pumpWidget(_host(PatientPeerChatPage(
        repo: repo,
        bookingRepo: BookingRepository(_api()),
        chatId: 'c2',
        requestId: 'r2',
        peerName: 'Neema K.',
        requestStatus: 'ACCEPTED',
        myUserId: 'me')));
    await tester.pumpAndSettle();
    expect(find.text('Neema K. (Peer Navigator)', findRichText: true),
        findsOneWidget);
    expect(find.text('Verified Peer'), findsOneWidget);
    expect(find.text('Trained Peer Navigator'), findsNothing);
    expect(find.text('Accepted'), findsOneWidget);
    expect(find.text('Peer support conversation active'), findsOneWidget);
    expect(find.text('Today'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-start-call')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-attach')), findsOneWidget);
    // Own message read by the peer: double ticks; none on the peer's.
    expect(find.byKey(const ValueKey('peer-ticks-m2-read')), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-ticks-m1-read')), findsNothing);
    expect(find.byKey(const ValueKey('peer-ticks-m1-sent')), findsNothing);
    double leftOf(String id) =>
        tester.getTopLeft(find.byKey(ValueKey('peer-message-$id'))).dx;
    expect(leftOf('m1'), lessThan(leftOf('m2')));

    await tester.enterText(
        find.byKey(const ValueKey('peer-message-input')), 'See you then');
    await tester.tap(find.byKey(const ValueKey('peer-send')));
    await tester.pumpAndSettle();
    expect(repo.sent, ['See you then']);
    expect(find.text('See you then'), findsOneWidget);
    expect(find.byKey(const ValueKey('peer-ticks-m3-sent')), findsOneWidget);
    // Stop the 10-second poll.
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('peer chat: the paperclip sends a photo', (tester) async {
    final repo = _Repo();
    await tester.pumpWidget(_host(PatientPeerChatPage(
        repo: repo,
        bookingRepo: BookingRepository(_api()),
        chatId: 'c2',
        requestId: 'r2',
        peerName: 'Neema K.',
        requestStatus: 'ACCEPTED',
        myUserId: 'me',
        pickImage: (_) async => (Uint8List.fromList([1, 2, 3]), 'image/png'))));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-attach')));
    await tester.pumpAndSettle();
    expect(find.text('Share a photo'), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('peer-attach-gallery')));
    await tester.pumpAndSettle();
    expect(repo.images, [('image/png', 3)]);
    expect(find.byKey(const ValueKey('peer-view-image-m4')), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('peer chat: session details from the menu', (tester) async {
    final repo = _Repo(requests: [_req('r2', 'ACCEPTED', chat: 'c2')]);
    await tester.pumpWidget(_host(PatientPeerChatPage(
        repo: repo,
        bookingRepo: BookingRepository(_api()),
        chatId: 'c2',
        requestId: 'r2',
        peerName: 'Neema K.',
        requestStatus: 'ACCEPTED',
        myUserId: 'me')));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey('peer-chat-more')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Session details'));
    await tester.pumpAndSettle();
    expect(find.text('Staying on track'), findsOneWidget);
    expect(find.textContaining('Tomorrow'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
  });

  testWidgets('the booking screen renders in Swahili', (tester) async {
    _tall(tester);
    await tester.pumpWidget(_host(
        BookPeerSupportPage(
            repo: _Repo(),
            bookingRepo: BookingRepository(_api()),
            nav: _nav,
            peer: _peer),
        language: 'sw'));
    await tester.pumpAndSettle();
    expect(find.text('Weka Msaada wa Rika'), findsOneWidget);
    expect(find.text('Omba Msaada'), findsOneWidget);
    expect(find.textContaining('Kesho'), findsOneWidget);
  });
}
