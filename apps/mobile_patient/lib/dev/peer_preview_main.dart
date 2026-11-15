// Debug-only design preview for the Patient Peer Navigator screens.
//
//   flutter run -t lib/dev/peer_preview_main.dart
//   --dart-define=PREVIEW_SCREEN=BOOK|SENT|CHAT|LIST (default: the module page)
//
// Shows labelled sample content (every name and message ends in "(sample)")
// answered in-process; nothing reaches a server. Refuses to run in release.
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../core/network/api_client.dart';
import '../core/theme/tokens.dart';
import '../features/patient/booking/booking_repository.dart';
import '../features/patient/modules/module_screens.dart';
import '../features/patient/modules/patient_modules.dart';
import '../features/patient/peer/peer_pages.dart';
import '../features/patient/peer/peer_repository.dart';
import '../l10n/app_localizations.dart';

const _screen = String.fromEnvironment('PREVIEW_SCREEN', defaultValue: '');
const _language =
    String.fromEnvironment('PREVIEW_LANGUAGE', defaultValue: 'en');

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

ApiClient _api() => ApiClient(
    dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1')),
    tokenStore: _NoTokens());

DateTime _at(int dayOffset, int hour, [int minute = 0]) {
  final n = DateTime.now();
  return DateTime(n.year, n.month, n.day + dayOffset, hour, minute);
}

const _neema = PeerNavigatorDto(
    id: 'p1', fullName: 'Neema K. (sample)', county: 'Nairobi');

class _PreviewRepo extends PatientPeerRepository {
  _PreviewRepo() : super(_api());

  @override
  Future<List<PeerNavigatorDto>> peers() async => [
        PeerNavigatorDto(
            id: 'p1',
            fullName: 'Neema K. (sample)',
            county: 'Nairobi',
            nextAvailable: _at(1, 10)),
        PeerNavigatorDto(
            id: 'p2',
            fullName: 'Brian O. (sample)',
            county: 'Kisumu',
            nextAvailable: _at(2, 13)),
      ];

  @override
  Future<List<PeerOpenSlotDto>> openSlots(String peerId) async => [
        for (final (d, h) in [
          (1, 10),
          (1, 11),
          (1, 14),
          (2, 10),
          (2, 13),
          (2, 15)
        ])
          PeerOpenSlotDto(id: 's$d$h', start: _at(d, h), end: _at(d, h, 10)),
      ];

  @override
  Future<List<PatientPeerRequestDto>> myRequests() async => [
        PatientPeerRequestDto(
            id: 'r1',
            peerId: 'p1',
            peerName: 'Neema K. (sample)',
            start: _at(1, 11),
            end: _at(1, 11, 10),
            supportType: 'CHAT',
            topic: 'Sticking to my PrEP (sample)',
            status: 'PENDING'),
        PatientPeerRequestDto(
            id: 'r2',
            peerId: 'p2',
            peerName: 'Brian O. (sample)',
            start: _at(0, 9),
            end: _at(0, 9, 10),
            supportType: 'CALL',
            topic: 'Staying motivated (sample)',
            status: 'ACCEPTED',
            conversationId: 'c2'),
      ];

  @override
  Future<PatientPeerRequestDto> request(
          {required String slotId,
          required String supportType,
          required String topic}) async =>
      _sent;

  @override
  Future<List<PeerChatMessageDto>> messages(String chatId) async => [
        PeerChatMessageDto(
            id: 'm1',
            senderId: 'me',
            content:
                "Hi Neema, thanks for accepting my request. I'd like to talk about sticking to my PrEP and some personal challenges. (sample)",
            sentAt: _at(0, 11, 2),
            hasImage: false,
            isRead: true),
        PeerChatMessageDto(
            id: 'm2',
            senderId: 'peer',
            content:
                "Hi Amina, I'm glad you reached out. Let's talk it through together. You're not alone. (sample)",
            sentAt: _at(0, 11, 4),
            hasImage: false),
        PeerChatMessageDto(
            id: 'm3',
            senderId: 'me',
            content:
                'Thank you! I really need some encouragement and tips. (sample)',
            sentAt: _at(0, 11, 6),
            hasImage: false,
            isRead: true),
      ];

  @override
  Future<void> markRead(String chatId) async {}
}

final _sent = PatientPeerRequestDto(
    id: 'r1',
    peerId: 'p1',
    peerName: 'Neema K. (sample)',
    start: _at(1, 11),
    end: _at(1, 11, 10),
    supportType: 'CHAT',
    topic:
        "I'd like to talk about sticking to my PrEP and some personal challenges. (sample)",
    status: 'PENDING');

void main() {
  if (kReleaseMode) {
    runApp(const MaterialApp(
        home: Scaffold(
            body: Center(
                child: Text('The preview is not available in release.')))));
    return;
  }
  final repo = _PreviewRepo();
  final booking = BookingRepository(_api());
  final nav = PatientShellNav(
      openServices: () {},
      openMessages: () {},
      openProfile: () {},
      select: (_) {});
  final Widget home = switch (_screen) {
    'LIST' => PeerNavigatorListPage(repo: repo, bookingRepo: booking, nav: nav),
    'BOOK' => BookPeerSupportPage(
        repo: repo, bookingRepo: booking, nav: nav, peer: _neema),
    'SENT' => PeerRequestSentPage(
        nav: nav, request: _sent, peerName: 'Neema K. (sample)'),
    'CHAT' => PatientPeerChatPage(
        repo: repo,
        bookingRepo: booking,
        chatId: 'c2',
        requestId: 'r2',
        peerName: 'Neema K. (sample)',
        requestStatus: 'ACCEPTED',
        myUserId: 'me'),
    _ => PeerNavigatorPage(
        nav: nav, repo: repo, bookingRepo: booking, myUserId: 'me'),
  };
  runApp(MaterialApp(
      debugShowCheckedModeBanner: false,
      theme: PatientTokens.theme(),
      locale: const Locale(_language),
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Banner(
          message: 'PREVIEW DATA',
          location: BannerLocation.topEnd,
          child: home)));
}
