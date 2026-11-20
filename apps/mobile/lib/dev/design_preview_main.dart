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
import '../core/video/join_eligibility.dart';
import '../features/doctor/agora_page.dart';
import '../features/doctor/call_close_out_page.dart';
import '../features/doctor/doctor_models.dart';
import '../features/doctor/doctor_pages.dart' show ChatThreadPage;
import '../features/doctor/doctor_cubit.dart';
import '../features/doctor/doctor_repository.dart';
import '../features/notifications/notification_cubit.dart';
import '../features/notifications/notification_repository.dart';
import '../features/peer/peer_pages.dart';
import '../features/peer/peer_repository.dart';
import '../l10n/app_localizations.dart';
import 'preview_agora_engine.dart';
import 'preview_avatar.dart';

/// `--dart-define=PREVIEW_VERIFICATION=PENDING` (or `REJECTED`) shows the
/// approval gate instead of the tabs.
const _verification =
    String.fromEnvironment('PREVIEW_VERIFICATION', defaultValue: 'VERIFIED');

/// `--dart-define=PREVIEW_ROLE=PEER` opens the Peer Navigator screens
/// instead of the Doctor ones. Those have no server behind them at all, so
/// everything they show is the sample content below.
const _role = String.fromEnvironment('PREVIEW_ROLE', defaultValue: 'DOCTOR');

/// `--dart-define=PREVIEW_RX=ACCEPT` lets the Smart Rx dispatch succeed (the
/// upload is discarded in-process). By default it is refused like every
/// other write, which shows the per-page "failed / retry" state.
const _rxMode = String.fromEnvironment('PREVIEW_RX', defaultValue: 'REFUSE');

/// `--dart-define=PREVIEW_SCREEN=CALL_ENDED` opens on the close-out a Doctor
/// sees right after ending a video call with "Sample Patient One". A real
/// call cannot run in the preview (no token server), so this starts where
/// the call screen hands over.
const _screen = String.fromEnvironment('PREVIEW_SCREEN', defaultValue: '');
// PREVIEW_SCREEN=SCHEDULE opens on the Consultation Schedule.
// PREVIEW_SCREEN=CHAT_ACTIVE / CHAT_EXPIRED opens a chat consultation before
// and after its booked time ends; PREVIEW_SCREEN=CALL opens a video call
// (stand-in engine, placeholder video) with the in-call chat button.

/// `--dart-define=PREVIEW_SLOTS=EMPTY` shows the schedule's empty state.
const _slotsMode = String.fromEnvironment('PREVIEW_SLOTS', defaultValue: '');

/// `--dart-define=PREVIEW_PEER=EMPTY` shows the Peer Navigator empty states.
const _peerMode = String.fromEnvironment('PREVIEW_PEER', defaultValue: '');

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  if (kReleaseMode) {
    runApp(const Directionality(
        textDirection: TextDirection.ltr,
        child: Center(
            child: Text('The design preview is not available in release.'))));
    return;
  }
  final store = _MemoryTokenStore();
  if (_role == 'PEER') {
    // Unsigned preview-only token so the peer chat can tell its own
    // messages (fixture sender 'me') apart. Never leaves the fixture adapter.
    String part(Map<String, Object> m) =>
        base64Url.encode(utf8.encode(jsonEncode(m))).replaceAll('=', '');
    await store.write(
        'access_token',
        '${part({'alg': 'none'})}.${part({
              'role': 'PEER',
              'user_id': 'me'
            })}.preview');
  }
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
                      // Top-left: the top-right corner holds the profile photo.
                      location: BannerLocation.topStart,
                      color: const Color(0xFFB45309),
                      child: child ?? const SizedBox()),
                  home: _role == 'PEER'
                      ? PeerApp(repo: PeerRepository(api))
                      : _screen == 'CALL_ENDED'
                          ? const _CallEndedPreview()
                          : _screen == 'SCHEDULE'
                              ? const _SchedulePreview()
                              : _screen.startsWith('CHAT_') ||
                                      _screen == 'CALL'
                                  ? const _PushedPreview()
                                  : const DoctorShell())))));
}

ChatDto _previewChat({required bool expired, String modality = 'CHAT'}) =>
    ChatDto(
        id: 'c2',
        appointmentId: 'a2',
        patientId: 'patient-a2',
        patientName: 'Sample Patient Two',
        modality: modality,
        appointmentStatus: 'CONFIRMED',
        lastMessageAt: _at(const Duration(minutes: -3)),
        expiresAt: _at(expired
            ? const Duration(minutes: -15)
            : const Duration(minutes: 8)),
        locked: false);

/// The Doctor shell with a chat consultation or a call pushed on top.
class _PushedPreview extends StatefulWidget {
  const _PushedPreview();
  @override
  State<_PushedPreview> createState() => _PushedPreviewState();
}

class _PushedPreviewState extends State<_PushedPreview> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await context.read<DoctorCubit>().loadAll();
      if (!mounted) return;
      final Widget page = _screen == 'CALL'
          ? AgoraPage(
              appointmentId: 'a1',
              engine: PreviewAgoraEngine(),
              requestPermissions: () async => true,
              loadJoin: () async => JoinEligibility(
                  eligible: true,
                  reason: 'OK',
                  joinOpenAt: DateTime.now().toUtc(),
                  joinCloseAt:
                      DateTime.now().toUtc().add(const Duration(minutes: 20)),
                  serverTime: DateTime.now().toUtc(),
                  clockOffset: Duration.zero),
              loadToken: () async => {
                    'app_id': 'preview',
                    'channel_name': 'preview',
                    'token': 'preview',
                    'uid': 1,
                    'expires_in': 3600,
                  },
              loadChat: () async => ChatThreadPage(
                  chat: _previewChat(expired: false, modality: 'VIDEO'),
                  inCall: true))
          : ChatThreadPage(
              chat: _previewChat(expired: _screen == 'CHAT_EXPIRED'));
      Navigator.of(context).push(MaterialPageRoute(builder: (_) => page));
    });
  }

  @override
  Widget build(BuildContext context) => const DoctorShell();
}

/// The Doctor shell switched to the Consultation Schedule.
class _SchedulePreview extends StatefulWidget {
  const _SchedulePreview();
  @override
  State<_SchedulePreview> createState() => _SchedulePreviewState();
}

class _SchedulePreviewState extends State<_SchedulePreview> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) DoctorShell.openSchedule(context);
    });
  }

  @override
  Widget build(BuildContext context) => const DoctorShell();
}

/// The Doctor shell with the post-call close-out pushed on top, as the call
/// screen leaves it.
class _CallEndedPreview extends StatefulWidget {
  const _CallEndedPreview();
  @override
  State<_CallEndedPreview> createState() => _CallEndedPreviewState();
}

class _CallEndedPreviewState extends State<_CallEndedPreview> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      final cubit = context.read<DoctorCubit>();
      await cubit.loadAll();
      if (!mounted) return;
      Navigator.of(context).push(MaterialPageRoute(
          builder: (_) => CallCloseOutPage(
              appointment: CallCloseOutPage.resolve(context, 'a1'))));
    });
  }

  @override
  Widget build(BuildContext context) => const DoctorShell();
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
    slot('s0', const Duration(minutes: -40), 'BOOKED',
        appointment: 'h1', name: 'Grace Wanjiku'),
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
  /// Outcomes recorded during this run (PREVIEW_RX=ACCEPT only).
  final _outcomes = <String, String>{};

  @override
  Future<ResponseBody> fetch(RequestOptions options,
      Stream<Uint8List>? requestStream, Future<void>? cancelFuture) async {
    final path = options.uri.path.replaceFirst('/api/v1', '');
    final query = options.uri.queryParameters;
    if (options.method == 'GET') {
      final photo = _photo(path);
      if (photo != null) {
        return ResponseBody.fromBytes(photo, 200, headers: {
          Headers.contentTypeHeader: ['image/png']
        });
      }
      final data = _read(path, query);
      if (data != null) return _json(200, {'success': true, 'data': data});
      return _json(404, _error('Not available in the design preview.'));
    }
    if (path == '/notifications/devices' || path.endsWith('/trace-ack')) {
      return _json(200, {'success': true, 'data': <String, dynamic>{}});
    }
    // Opt-in only, so the Rx dispatch and post-call close-out outcomes can
    // be seen. Photo bytes are discarded; the outcome lives in memory for
    // this run only and mirrors the server's rules.
    final consultation = RegExp(
            r'^/consultations/([^/]+)/(prescription|complete|complete-no-rx)$')
        .firstMatch(path);
    if (_rxMode == 'ACCEPT' && consultation != null) {
      final id = consultation.group(1)!;
      await Future<void>.delayed(const Duration(milliseconds: 600));
      final recorded = _outcomes[id];
      switch (consultation.group(2)) {
        case 'prescription':
          if (recorded == 'COMPLETED_WITH_RX' ||
              recorded == 'COMPLETED_NO_RX') {
            return _json(
                409,
                _error(
                    'The prescription for this consultation has already been dispatched.'));
          }
          return _json(201, {
            'success': true,
            'data': {'id': 'preview-rx', 'status': 'PREVIEW_DISCARDED'}
          });
        case 'complete-no-rx':
          if (recorded != null) {
            return _json(
                409,
                _error(
                    'A prescription decision already exists for this consultation.'));
          }
          _outcomes[id] = 'COMPLETED_NO_RX';
          return _json(200, {'success': true, 'data': <String, dynamic>{}});
        default:
          final outcome = (jsonDecode(options.data is String
                  ? options.data as String
                  : jsonEncode(options.data)) as Map)['outcome']
              ?.toString();
          if (recorded != null && recorded != outcome) {
            return _json(409, _error('Already completed.'));
          }
          _outcomes[id] = outcome ?? 'COMPLETED_WITH_RX';
          return _json(200, {'success': true, 'data': <String, dynamic>{}});
      }
    }
    return _json(503, _error('Design preview: this action is not connected.'));
  }

  /// Sample profile photos (drawn silhouettes, see preview_avatar.dart):
  /// the signed-in doctor or peer, and the other person in each chat.
  Uint8List? _photo(String path) {
    if (path == '/doctor/profile/photo') return previewAvatarPng(1);
    if (path == '/peer/profile/photo') return previewAvatarPng(0);
    final other =
        RegExp(r'^/(appointments|peer-requests)/([^/]+)/counterpart-photo$')
            .firstMatch(path);
    if (other != null) return previewAvatarPng(other.group(2)!.hashCode + 2);
    return null;
  }

  Object? _read(String path, Map<String, String> query) {
    final peer = _peerRead(path, query);
    if (peer != null) return peer;
    if (path == '/doctor/profile') return _profile;
    if (path == '/doctor/duty') return {'is_on_duty': true};
    if (path == '/doctor/appointments') {
      final completedNow = [
        for (final a in _queue())
          if (_outcomes.containsKey(a['id']))
            {
              ...a,
              'status': 'COMPLETED',
              'clinical_outcome': _outcomes[a['id']],
              'completed_at': _at(Duration.zero),
              'queue_actionable': false,
            }
      ];
      return query['view'] == 'history'
          ? [...completedNow, ..._history()]
          // The server's complete list carries past encounters too.
          : [
              for (final a in _queue())
                if (!_outcomes.containsKey(a['id'])) a,
              ...completedNow,
              for (final h in _history()) {...h, 'queue_actionable': false}
            ];
    }
    if (path == '/doctor/slots') {
      return _slotsMode == 'EMPTY' ? <Object>[] : _slots();
    }
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

// ---------------------------------------------------------------------------
// Peer Navigator fixtures (PREVIEW_ROLE=PEER). Sample content only.
// ---------------------------------------------------------------------------

String _iso(Duration fromNow) =>
    DateTime.now().add(fromNow).toUtc().toIso8601String();

final _peerRequests = [
  {
    'id': 'pr-1',
    'patient_display_name': 'Amina M.',
    'patient_age_range': '15-19',
    'start_time': _iso(const Duration(hours: 1)),
    'end_time': _iso(const Duration(hours: 1, minutes: 10)),
    'support_type': 'CHAT',
    'topic': 'Staying on track with my health goals (sample)',
    'status': 'PENDING',
    'conversation_id': null,
  },
  {
    'id': 'pr-2',
    'patient_display_name': 'Peter K.',
    'patient_age_range': '20-24',
    'start_time': _todayAt(10, 40),
    'end_time': _todayAt(10, 50),
    'support_type': 'CALL',
    'topic': 'Follow-up and motivation (sample)',
    'status': 'PENDING',
    'conversation_id': null,
  },
  {
    'id': 'pr-3',
    'patient_display_name': 'Neema K.',
    'patient_age_range': '20-24',
    'start_time': _todayAt(10, 10),
    'end_time': _todayAt(10, 20),
    'support_type': 'CHAT',
    'topic': 'Tips for daily routine and wellbeing (sample)',
    'status': 'ACCEPTED',
    'conversation_id': 'pc-1',
  },
];

String _todayAt(int h, int m) {
  final n = DateTime.now();
  return DateTime(n.year, n.month, n.day, h, m).toUtc().toIso8601String();
}

Map<String, Object?> _peerNotice(String id, String type, String title,
        String body, String entityType, String entity, Duration ago,
        {bool read = false}) =>
    {
      'id': id,
      'user_id': 'me',
      'notification_type': type,
      'title': title,
      'body': body,
      'created_at': _iso(-ago),
      'delivery_status': 'SENT',
      'related_entity_type': entityType,
      'related_entity_id': entity,
      'read_at': read ? _iso(-ago) : null,
    };

Object? _peerRead(String path, Map<String, String> query) {
  if (_role != 'PEER') return null;
  const empty = _peerMode == 'EMPTY';
  if (path == '/notifications') {
    if (empty ||
        query.containsKey('before_id') ||
        query.containsKey('after_id')) {
      return <Object>[];
    }
    return [
      _peerNotice(
          'pn-1',
          'PEER_REQUEST_NEW',
          'New support request',
          'Someone has requested a support session with you. Open it to accept or decline.',
          'PEER_REQUEST',
          'pr-1',
          const Duration(minutes: 5)),
      _peerNotice(
          'pn-2',
          'NEW_CHAT_MESSAGE',
          'New message',
          'You have a new message in your peer support chat.',
          'PEER_CHAT',
          'pc-1',
          const Duration(minutes: 28)),
      _peerNotice(
          'pn-3',
          'PEER_REQUEST_CANCELLED',
          'Request cancelled',
          'A support request was cancelled and your slot is open again.',
          'PEER_REQUEST',
          'pr-9',
          const Duration(days: 1, hours: 2),
          read: true),
    ];
  }
  if (path == '/notifications/unread-count') {
    return {'unread_count': empty ? 0 : 2};
  }
  if (empty &&
      (path == '/peer/requests' ||
          path == '/peer-chats' ||
          path == '/peer/slots')) {
    return <Object>[];
  }
  if (path == '/peer/profile') {
    return {
      'id': 'peer-1',
      'full_name': 'Grace Wanjiku (sample)',
      'county': 'Nairobi',
      'organization': 'Kenyatta Youth Centre (sample)',
      'bio': null,
      'approval_status': 'APPROVED',
    };
  }
  if (path == '/peer/requests') {
    final status = query['status'];
    return status == null
        ? _peerRequests
        : _peerRequests.where((r) => r['status'] == status).toList();
  }
  if (path == '/peer/slots') {
    final from = DateTime.parse(query['from'] ?? _iso(Duration.zero)).toLocal();
    DateTime at(int h, int m) =>
        DateTime(from.year, from.month, from.day, h, m);
    return [
      for (final (id, h, m, status) in [
        ('ps-1', 10, 0, 'AVAILABLE'),
        ('ps-2', 10, 10, 'BOOKED'),
        ('ps-3', 10, 20, 'AVAILABLE'),
        ('ps-4', 10, 40, 'REQUESTED'),
      ])
        {
          'id': id,
          'peer_id': 'peer-1',
          'status': status,
          'start_time': at(h, m).toUtc().toIso8601String(),
          'end_time': at(h, m + 10).toUtc().toIso8601String(),
        }
    ];
  }
  if (path == '/peer-chats') {
    return [
      {
        'id': 'pc-1',
        'request_id': 'pr-3',
        'counterpart_name': 'Neema K.',
        'support_type': 'CHAT',
        'request_status': 'ACCEPTED',
        'start_time': _iso(const Duration(minutes: 20)),
        'last_message':
            'That would be great. I sometimes find it hard to stay consistent. (sample)',
        'last_message_at': _iso(const Duration(minutes: -3)),
        'unread_count': 2,
      },
    ];
  }
  if (path == '/peer-chats/pc-1/messages') {
    Map<String, Object?> m(
            String id, String sender, String text, int minutesAgo) =>
        {
          'id': id,
          'conversation_id': 'pc-1',
          'sender_id': sender,
          'content': text,
          'is_read': true,
          'sent_at': _iso(Duration(minutes: -minutesAgo)),
          'attachment': null,
        };
    return [
      m(
          'm1',
          'patient',
          'Hi! Thanks for accepting my request. I\'d like to talk about staying on track with my health goals. (sample)',
          9),
      m(
          'm2',
          'me',
          'Hi! I\'m glad you reached out. Let\'s talk about what\'s working for you and where you might need some support. (sample)',
          7),
      m(
          'm3',
          'patient',
          'That would be great. I sometimes find it hard to stay consistent. (sample)',
          3),
    ];
  }
  if (RegExp(r'^/peer-requests/[^/]+/call$').hasMatch(path)) {
    final now = DateTime.now().toUtc();
    return {
      'join': {
        'eligible': false,
        'reason': 'NOT_YET_OPEN',
        'join_open_at': now.add(const Duration(minutes: 5)).toIso8601String(),
        'join_close_at': now.add(const Duration(minutes: 40)).toIso8601String(),
        'server_time': now.toIso8601String(),
      },
      'session_status': null,
    };
  }
  return null;
}
