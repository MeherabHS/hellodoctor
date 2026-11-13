// DESIGN PREVIEW ENTRY POINT — never the shipped app.
//
// Opens the signed-in Doctor screens directly, with no login, no emailed
// code and no admin-approval wait, so UI work can be seen on the emulator
// without a seeded backend. It does this WITHOUT touching the real sign-in
// path: `lib/main.dart` and `lib/app.dart` are unchanged, and no backend
// check is relaxed. Every network call is answered in-process by
// [_FixtureAdapter] below; nothing leaves the device and nothing is saved.
//
// Run:  flutter run -t lib/dev/design_preview_main.dart
//
// All names and content here are invented fixtures. Refuses to start in a
// release build.
import 'dart:convert';

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../app.dart';
import '../core/l10n/doctor_locale.dart';
import '../core/network/api_client.dart';
import '../core/network/screen_refresh.dart';
import '../core/theme/tokens.dart';
import '../features/auth/auth_cubit.dart';
import '../features/doctor/doctor_cubit.dart';
import '../features/doctor/doctor_repository.dart';
import '../features/notifications/notification_cubit.dart';
import '../features/notifications/notification_repository.dart';
import '../features/doctor/doctor_models.dart';
import '../features/peer/peer_pages.dart';
import '../l10n/app_localizations.dart';

/// `--dart-define=PREVIEW_VERIFICATION=PENDING` (or `REJECTED`) shows the
/// approval gate instead of the tabs.
const _verification =
    String.fromEnvironment('PREVIEW_VERIFICATION', defaultValue: 'VERIFIED');

/// `--dart-define=PREVIEW_ROLE=PEER` opens the Peer Navigator screens
/// instead of the Doctor ones. Those have no server behind them at all, so
/// everything they show is the sample content below.
const _role = String.fromEnvironment('PREVIEW_ROLE', defaultValue: 'DOCTOR');

PeerData _peerData() {
  final chat = ChatDto.fromJson(_chats().first);
  PeerThread thread(String name, String state, String topic, String preview,
          String when, int unread) =>
      PeerThread(
          name: name,
          state: state,
          topic: topic,
          preview: preview,
          when: when,
          unread: unread,
          chat: chat);
  PeerClient client(
          String name, String id, String phone, String status, String next) =>
      PeerClient(
          name: name,
          id: id,
          phone: phone,
          status: status,
          next: next,
          location: 'Nairobi, Kenya');
  return PeerData(
      name: 'Grace Wanjiku (sample)',
      location: 'Nairobi, Kenya',
      peerId: 'PN-0000',
      bio:
          'Community peer navigator supporting HIV prevention, testing, treatment adherence and linkage to care.',
      approved: true,
      stats: const {
        'Support Requests': '12',
        'Active Conversations': '8',
        'Calls Completed': '3',
        'People Supported': '48',
        'Support Conversations': '36',
        'Referrals Made': '12',
        'Client Follow-up Rate': '92%',
        'New (This Week)': '6',
      },
      requests: const [
        PeerRequest(
            'John Mwangi', 'Request for peer support', '10 minutes ago'),
      ],
      clients: [
        client('Jane Mwangi', 'AMK-10045', '0712 345 678', 'Active',
            'Next: Follow-up 20 Jan'),
        client('Daniel Otieno', 'AMK-10046', '0720 987 654', 'Needs Follow-up',
            'Due: 15 Jan'),
        client('Alice Wanjiku', 'AMK-10047', '0701 234 567', 'In Support',
            'Ongoing support'),
        client('Peter Kamau', 'AMK-10048', '0722 456 789', 'On Hold',
            'Returned: 10 Jan'),
        client('Susan Muthoni', 'AMK-10049', '0790 321 098', 'Active',
            'Next: Follow-up 25 Jan'),
      ],
      threads: [
        thread('Juma Mwangi', 'Active', 'Ongoing support',
            'Sample message preview (fixture).', '10:24 AM', 2),
        thread('Amina Njeri', 'Active', 'Follow-up',
            'Sample message preview (fixture).', '9:18 AM', 1),
        thread('Peter Kamau', 'Active', 'Service linkage',
            'Sample message preview (fixture).', 'Yesterday', 0),
        thread('Stella Nduku', 'Closed', 'Support completed',
            'Sample message preview (fixture).', 'Nov 12', 0),
      ]);
}

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  if (kReleaseMode) {
    runApp(const Directionality(
        textDirection: TextDirection.ltr,
        child: Center(
            child: Text('The design preview is not available in release.'))));
    return;
  }
  final store = _MemoryTokenStore();
  final api = ApiClient(
      dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1'))
        ..httpClientAdapter = _FixtureAdapter(),
      tokenStore: store);
  final locale = DoctorLocaleController(store);
  runApp(DoctorLocaleScope(
      controller: locale,
      child: MultiBlocProvider(
          providers: [
            BlocProvider<AuthCubit>(create: (_) => AuthCubit(api)),
            BlocProvider<DoctorCubit>(
                create: (_) => DoctorCubit(DoctorRepository(api))),
            BlocProvider<NotificationCubit>(
                create: (_) => NotificationCubit(NotificationRepository(api))),
          ],
          child: ListenableBuilder(
              listenable: locale,
              builder: (_, __) => MaterialApp(
                  locale: locale.locale,
                  localizationsDelegates:
                      AppLocalizations.localizationsDelegates,
                  supportedLocales: AppLocalizations.supportedLocales,
                  navigatorObservers: [screenRouteObserver],
                  debugShowCheckedModeBanner: false,
                  theme: DoctorTokens.theme(),
                  builder: (_, child) => Banner(
                      message: 'PREVIEW DATA',
                      location: BannerLocation.topEnd,
                      color: const Color(0xFFB45309),
                      child: child ?? const SizedBox()),
                  home: _role == 'PEER'
                      ? PeerShell(data: _peerData())
                      : const DoctorShell())))));
}

class _MemoryTokenStore implements TokenStore {
  final _values = <String, String>{};
  @override
  Future<String?> read(String key) async => _values[key];
  @override
  Future<void> write(String key, String value) async => _values[key] = value;
  @override
  Future<void> delete(String key) async => _values.remove(key);
  @override
  Future<void> clear() async => _values.clear();
}

String _at(Duration fromNow) =>
    DateTime.now().toUtc().add(fromNow).toIso8601String();

Map<String, dynamic> _appointment(String id, String name, String modality,
        String status, Duration start, String complaint,
        {Map<String, dynamic>? join, String? conversation}) =>
    {
      'id': id,
      'appointment_number': 'PREVIEW-$id',
      'patient_id': 'patient-$id',
      'patient_display_name': name,
      'modality': modality,
      'status': status,
      'start_time': _at(start),
      'end_time': _at(start + const Duration(minutes: 10)),
      'chief_complaint': complaint,
      'conversation_id': conversation,
      'chat_expires_at': _at(const Duration(hours: 20)),
      'chat_locked': false,
      'queue_actionable': true,
      if (join != null) 'join': join,
    };

List<Map<String, dynamic>> _queue() => [
      _appointment('a1', 'Sample Patient One', 'VIDEO', 'CONFIRMED',
          const Duration(minutes: 5), 'Sample reason for visit (fixture)',
          conversation: 'c1'),
      _appointment('a2', 'Sample Patient Two', 'CHAT', 'CONFIRMED',
          const Duration(minutes: 25), 'Sample follow-up question (fixture)',
          conversation: 'c2'),
      _appointment('a3', 'Sample Patient Three', 'VIDEO', 'CONFIRMED',
          const Duration(hours: 2), 'Sample reason for visit (fixture)',
          join: {
            'eligible': false,
            'reason': 'NOT_YET_OPEN',
            'join_open_at': _at(const Duration(hours: 1, minutes: 50)),
            'join_close_at': _at(const Duration(hours: 2, minutes: 10)),
            'server_time': _at(Duration.zero),
          }),
    ];

/// `--dart-define=PREVIEW_HISTORY=EMPTY` shows the Consultation History
/// empty state.
const _historyMode =
    String.fromEnvironment('PREVIEW_HISTORY', defaultValue: 'PILED');

/// A completed consultation at a fixed time (UTC), for the history list.
Map<String, dynamic> _done(
        String id, String patient, String name, String modality, String utc) =>
    {
      'id': id,
      'appointment_number': 'PREVIEW-$id',
      'patient_id': patient,
      'patient_display_name': name,
      'modality': modality,
      'status': 'COMPLETED',
      'start_time': utc,
      'end_time': DateTime.parse(utc)
          .add(const Duration(minutes: 10))
          .toIso8601String(),
      'completed_at': utc,
      'conversation_id': 'c-$id',
      'chat_locked': true,
      'queue_actionable': false,
    };

/// The piled-up list of the history reference image
/// (docs/design/doctor-consultation-history-list-reference.png); names are
/// fixtures taken from that image.
List<Map<String, dynamic>> _history() => _historyMode == 'EMPTY'
    ? []
    : [
        _done('h1', 'pg', 'Grace Wanjiku', 'VIDEO', '2026-11-13T05:33:00Z'),
        _done('h2', 'pj', 'John Ondimu', 'CHAT', '2026-11-11T13:20:00Z'),
        _done('h3', 'pp', 'Peter Mwangi', 'VIDEO', '2026-11-08T08:05:00Z'),
        _done('h4', 'pg', 'Grace Wanjiku', 'CHAT', '2026-11-02T07:00:00Z'),
        _done('h5', 'pg', 'Grace Wanjiku', 'VIDEO', '2026-10-20T07:00:00Z'),
        _done('h6', 'pp', 'Peter Mwangi', 'CHAT', '2026-10-30T07:00:00Z'),
        _done('h7', 'pp', 'Peter Mwangi', 'VIDEO', '2026-10-15T07:00:00Z'),
        _done('h8', 'pp', 'Peter Mwangi', 'VIDEO', '2026-09-28T07:00:00Z'),
      ];

List<Map<String, dynamic>> _slots() {
  Map<String, dynamic> slot(String id, Duration start, String status,
          {String? appointment, String? name}) =>
      {
        'id': id,
        'status': status,
        'bookable': status == 'AVAILABLE',
        'start_time': _at(start),
        'end_time': _at(start + const Duration(minutes: 10)),
        'appointment_id': appointment,
        'patient_display_name': name,
      };
  return [
    slot('s1', const Duration(minutes: 5), 'BOOKED',
        appointment: 'a1', name: 'Sample Patient One'),
    slot('s2', const Duration(minutes: 25), 'BOOKED',
        appointment: 'a2', name: 'Sample Patient Two'),
    slot('s3', const Duration(minutes: 45), 'AVAILABLE'),
    slot('s4', const Duration(minutes: 55), 'AVAILABLE'),
    slot('s5', const Duration(minutes: 65), 'BLOCKED'),
  ];
}

List<Map<String, dynamic>> _chats() => [
      {
        'id': 'c2',
        'appointment_id': 'a2',
        'patient_id': 'patient-a2',
        'patient_display_name': 'Sample Patient Two',
        'last_message_at': _at(const Duration(minutes: -4)),
        'expires_at': _at(const Duration(hours: 20)),
        'is_locked': false,
        'modality': 'CHAT',
        'appointment_status': 'CONFIRMED',
      },
      {
        'id': 'c9',
        'appointment_id': 'h2',
        'patient_id': 'patient-h2',
        'patient_display_name': 'Sample Patient Five',
        'last_message_at': _at(const Duration(days: -5)),
        'expires_at': _at(const Duration(days: -4)),
        'is_locked': true,
        'modality': 'CHAT',
        'appointment_status': 'COMPLETED',
      },
    ];

List<Map<String, dynamic>> _messages() => [
      {
        'id': 'm1',
        'content': 'Sample patient message (fixture).',
        'sent_at': _at(const Duration(minutes: -6)),
        'sender_id': 'preview-patient',
      },
      {
        'id': 'm2',
        'content': 'Sample doctor reply (fixture).',
        'sent_at': _at(const Duration(minutes: -4)),
        'sender_id': 'preview-doctor-user',
      },
    ];

List<Map<String, dynamic>> _questions() => [
      {
        'id': 'q1',
        'question_text': 'Sample anonymous health question (fixture)?',
        'status': 'OPEN',
        'category': 'General',
        'created_at': _at(const Duration(hours: -3)),
      },
      {
        'id': 'q2',
        'question_text': 'Sample answered question (fixture)?',
        'status': 'ANSWERED',
        'category': 'General',
        'created_at': _at(const Duration(days: -1)),
        'answer_text': 'Sample published answer (fixture).',
        'answering_doctor': 'Dr. Preview Fixture',
        'answered_at': _at(const Duration(hours: -20)),
      },
    ];

List<Map<String, dynamic>> _notifications() => [
      {
        'id': 'n1',
        'user_id': 'preview-doctor-user',
        'notification_type': 'BOOKING_CONFIRMED',
        'title': 'New appointment',
        'body': 'A consultation was booked (fixture).',
        'delivery_status': 'DELIVERED',
        'created_at': _at(const Duration(minutes: -5)),
      },
      {
        'id': 'n2',
        'user_id': 'preview-doctor-user',
        'notification_type': 'NEW_CHAT_MESSAGE',
        'title': 'Patient message',
        'body': 'You have a new message (fixture).',
        'delivery_status': 'DELIVERED',
        'created_at': _at(const Duration(minutes: -30)),
        'read_at': _at(const Duration(minutes: -10)),
      },
    ];

const _profile = {
  'user_id': 'preview-doctor-user',
  'full_name': 'Dr. Preview Fixture',
  'license_number': 'PREVIEW-0000',
  'current_hospital': 'Sample Hospital (fixture)',
  'primary_specialty': 'General Practice',
  'professional_verification_status': _verification,
  'license_authority': 'KMPDC',
  'license_country': 'KE',
  'qualifications': ['MBChB (fixture)'],
};

/// Answers every request locally. Reads return fixtures; anything unknown is
/// a 404 so the screen shows its own error or empty state; every write is
/// refused so the preview can never look as if it saved something.
class _FixtureAdapter implements HttpClientAdapter {
  @override
  Future<ResponseBody> fetch(RequestOptions options,
      Stream<Uint8List>? requestStream, Future<void>? cancelFuture) async {
    final path = options.uri.path.replaceFirst('/api/v1', '');
    final query = options.uri.queryParameters;
    if (options.method == 'GET') {
      final data = _read(path, query);
      if (data != null) return _json(200, {'success': true, 'data': data});
      return _json(404, _error('Not available in the design preview.'));
    }
    if (path == '/notifications/devices' || path.endsWith('/trace-ack')) {
      return _json(200, {'success': true, 'data': <String, dynamic>{}});
    }
    return _json(503, _error('Design preview: this action is not connected.'));
  }

  Object? _read(String path, Map<String, String> query) {
    if (path == '/doctor/profile') return _profile;
    if (path == '/doctor/duty') return {'is_on_duty': true};
    if (path == '/doctor/appointments') {
      return query['view'] == 'history'
          ? _history()
          // The server's complete list carries past encounters too.
          : [
              ..._queue(),
              for (final h in _history()) {...h, 'queue_actionable': false}
            ];
    }
    if (path == '/doctor/slots') return _slots();
    if (path == '/doctor/chats') return _chats();
    if (path == '/doctor/qna/questions') return _questions();
    if (path == '/config/booking-policy') return {'payment_required': false};
    // Zeroed: the launch runs with the financial module deferred, so the
    // Wallet tab shows its "not yet active" notice rather than amounts.
    if (path == '/doctor/wallet/summary') {
      return {
        'doctor_name': 'Dr. Preview Fixture',
        'currency': 'KES',
        'total_gross': '0.00',
        'withheld_fee': '0.00',
        'final_net': '0.00',
        'platform_charge_percent': '20',
      };
    }
    if (path == '/notifications') {
      return query.containsKey('before_id') || query.containsKey('after_id')
          ? <Object>[]
          : _notifications();
    }
    if (path == '/notifications/unread-count') return {'unread_count': 1};
    if (RegExp(r'^/chat/[^/]+/messages$').hasMatch(path)) {
      return query.containsKey('before') || query.containsKey('after')
          ? <Object>[]
          : _messages();
    }
    final conversation =
        RegExp(r'^/appointments/([^/]+)/conversation$').firstMatch(path);
    if (conversation != null) {
      final id = conversation.group(1);
      return _chats().firstWhere((c) => c['appointment_id'] == id,
          orElse: () => {
                ..._chats().first,
                'id': 'c-$id',
                'appointment_id': id,
                'modality': 'VIDEO'
              });
    }
    if (RegExp(r'^/doctor/consultations/[^/]+/summary$').hasMatch(path)) {
      return {
        'chief_complaint': 'Sample reason for visit (fixture)',
        'clinical_notes': 'Sample intake note written by the patient (fixture)',
        'documents': <Object>[],
        'prescriptions': <Object>[],
      };
    }
    if (RegExp(r'^/doctor/appointments/[^/]+/patient-history$')
        .hasMatch(path)) {
      return <Object>[];
    }
    if (path == '/doctor/articles') return <Object>[];
    return null;
  }

  Map<String, dynamic> _error(String message) => {
        'success': false,
        'error': {'code': 'PREVIEW', 'message': message}
      };

  ResponseBody _json(int status, Object body) =>
      ResponseBody.fromBytes(utf8.encode(jsonEncode(body)), status, headers: {
        Headers.contentTypeHeader: [Headers.jsonContentType]
      });

  @override
  void close({bool force = false}) {}
}
