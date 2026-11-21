// DESIGN PREVIEW ENTRY POINT — never the shipped app.
//
// Opens the signed-in patient shell (Home, Services, Messages, Profile)
// directly, with labelled sample consultations and notifications, so the
// screens can be seen on the emulator without a seeded backend or signing
// in. `lib/main.dart` and `lib/app.dart` are unchanged and no backend check
// is relaxed: every network call is answered in-process by [_FixtureAdapter]
// below, and nothing leaves the device. Reminders and push registration are
// not started.
//
// Run:  flutter run -t lib/dev/design_preview_main.dart
//       --dart-define=PREVIEW_LANGUAGE=sw   (Swahili)
//
// All names and content are invented fixtures. Refuses to start in a
// release build.
import 'dart:convert';

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:provider/provider.dart';

import '../app.dart';
import '../core/cache/patient_cache_store.dart';
import '../core/l10n/locale_controller.dart';
import '../core/network/api_client.dart';
import '../core/network/connectivity_service.dart';
import '../core/theme/tokens.dart';
import '../features/notifications/notification_cubit.dart';
import '../features/notifications/notification_navigation.dart';
import '../features/notifications/notification_repository.dart';
import '../features/patient/home/home_cubit.dart';
import '../features/patient/home/home_repository.dart';
import '../features/patient/profile/family_profiles_cubit.dart';
import '../features/patient/profile/family_profiles_repository.dart';
import '../features/patient/profile/profile_cubit.dart';
import '../features/patient/profile/profile_repository.dart';
import '../l10n/app_localizations.dart';
import 'preview_avatar.dart';

const _language =
    String.fromEnvironment('PREVIEW_LANGUAGE', defaultValue: 'en');

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

const _profile = {
  'id': 'preview-patient-profile',
  'user_id': 'preview-patient-user',
  'full_name': 'Amina Preview',
  'date_of_birth': '2004-03-14',
  'gender': 'FEMALE',
};

Map<String, Object?> _join(Duration opensIn, Duration closesIn) => {
      'eligible': opensIn <= Duration.zero && closesIn > Duration.zero,
      'reason': opensIn > Duration.zero ? 'TOO_EARLY' : 'OK',
      'join_open_at': _at(opensIn),
      'join_close_at': _at(closesIn),
      'server_time': _at(Duration.zero),
    };

Map<String, Object?> _appointment(
  String id,
  String doctor,
  String specialty,
  String modality,
  String status,
  Duration startsIn, {
  Duration length = const Duration(minutes: 30),
  String? outcome,
}) {
  final end = startsIn + length;
  return {
    'id': id,
    'appointment_number': 'PRV-${id.toUpperCase()}',
    'doctor_id': 'doctor-$id',
    'doctor_display_name': doctor,
    'primary_specialty': specialty,
    'modality': modality,
    'status': status,
    'payment_policy': 'NOT_REQUIRED',
    'start_time': _at(startsIn),
    'end_time': _at(end),
    if (modality == 'CHAT') 'chat_expires_at': _at(end),
    if (modality == 'CHAT') 'chat_locked': end <= Duration.zero,
    'join': _join(startsIn - const Duration(minutes: 15), end),
    if (outcome != null) 'clinical_outcome': outcome,
    if (outcome != null) 'completed_at': _at(end),
  };
}

/// Upcoming: a video visit in 10 minutes (inside the 15-minute window, so
/// Home shows its Upcoming Consultation card) and a chat later today.
/// Active: a chat consultation running now. Past: completed visits.
List<Map<String, Object?>> _appointments(String? status) {
  final upcoming = [
    _appointment('a1', 'Grace Wanjiru', 'General Practice (sample)', 'VIDEO',
        'CONFIRMED', const Duration(minutes: 10)),
    _appointment('a2', 'Daniel Otieno', 'Sexual & Reproductive Health (sample)',
        'CHAT', 'CONFIRMED', const Duration(hours: 5)),
  ];
  final active = [
    _appointment('a3', 'Faith Achieng', 'Adolescent Health (sample)', 'CHAT',
        'IN_CONSULTATION', const Duration(minutes: -10),
        length: const Duration(minutes: 40)),
  ];
  final past = [
    _appointment('a4', 'Peter Kamau', 'General Practice (sample)', 'CHAT',
        'COMPLETED', const Duration(days: -2),
        outcome: 'COMPLETED_NO_RX'),
    _appointment('a5', 'Grace Wanjiru', 'General Practice (sample)', 'VIDEO',
        'COMPLETED', const Duration(days: -6),
        outcome: 'COMPLETED_WITH_RX'),
    _appointment('a6', 'Mercy Njeri', 'HIV Care (sample)', 'CHAT', 'COMPLETED',
        const Duration(days: -12),
        outcome: 'REFERRED'),
  ];
  return switch (status) {
    'upcoming' => [...active, ...upcoming],
    'past' => past,
    _ => [...active, ...upcoming, ...past],
  };
}

/// A short sample exchange, oldest first.
List<Map<String, Object?>> _messages() {
  Map<String, Object?> m(String id, bool mine, String text, int minutesAgo) => {
        'id': id,
        'sender_id': mine ? 'preview-patient-user' : 'doctor-sample',
        'content': text,
        'sent_at': _at(Duration(minutes: -minutesAgo)),
      };
  return [
    m('m1', true,
        'Hello doctor, I have had a headache since yesterday. (sample)', 9),
    m(
        'm2',
        false,
        'Hello Amina. Thank you for reaching out. Have you had a fever or any nausea? (sample)',
        8),
    m('m3', true, 'No fever, just tired and the headache. (sample)', 6),
    m(
        'm4',
        false,
        'Drink plenty of water and rest today. If it gets worse, book a video visit. (sample)',
        3),
  ];
}

Map<String, Object?> _notice(
        String id, String type, String title, String body, Duration ago,
        {bool read = false}) =>
    {
      'id': id,
      'user_id': 'preview-patient-user',
      'notification_type': type,
      'title': title,
      'body': body,
      'delivery_status': 'DELIVERED',
      'created_at': _at(-ago),
      if (read) 'read_at': _at(-ago),
    };

final _notifications = [
  _notice(
      'n1',
      'CONSULTATION_REMINDER_15M',
      'Consultation in 15 minutes',
      'Your video consultation starts soon. Open the waiting room. (sample)',
      const Duration(minutes: 4)),
  _notice('n2', 'NEW_CHAT_MESSAGE', 'New message from your doctor',
      'Open the chat to read it. (sample)', const Duration(minutes: 38)),
  _notice(
      'n3',
      'PRESCRIPTION_AVAILABLE',
      'Prescription ready',
      'Your doctor has sent a prescription to Health Records. (sample)',
      const Duration(hours: 3),
      read: true),
  _notice('n4', 'BOOKING_CONFIRMED', 'Booking confirmed',
      'Your chat consultation is booked. (sample)', const Duration(days: 1),
      read: true),
  _notice(
      'n5',
      'PEER_REQUEST_ACCEPTED',
      'Peer support accepted',
      'Your Peer Navigator accepted your request. (sample)',
      const Duration(days: 3),
      read: true),
];

/// Answers every request in-process. Unknown reads get an empty list;
/// writes succeed with an empty object.
class _FixtureAdapter implements HttpClientAdapter {
  @override
  Future<ResponseBody> fetch(RequestOptions options,
      Stream<Uint8List>? requestStream, Future<void>? cancelFuture) async {
    final path = options.uri.path.replaceFirst('/api/v1', '');
    final query = options.uri.queryParameters;
    // Photos (own and counterpart) are a drawn sample silhouette.
    if (options.method == 'GET' &&
        (path.endsWith('/photo') || path.endsWith('/counterpart-photo'))) {
      return ResponseBody.fromBytes(previewAvatarPng(path.hashCode), 200,
          headers: {
            Headers.contentTypeHeader: ['image/png']
          });
    }
    Object? body;
    final conversation =
        RegExp(r'^/appointments/([^/]+)/conversation$').firstMatch(path);
    final messages = RegExp(r'^/chat/([^/]+)/messages$').firstMatch(path);
    final single = RegExp(r'^/appointments/([^/]+)$').firstMatch(path);
    if (options.method == 'GET' && single != null) {
      body = _appointments(null).firstWhere((a) => a['id'] == single.group(1),
          orElse: () => _appointments(null).first);
    } else if (options.method == 'GET' && conversation != null) {
      final id = conversation.group(1)!;
      final a = _appointments(null).firstWhere((a) => a['id'] == id,
          orElse: () => _appointments(null).first);
      body = {
        'id': 'conv-$id',
        'appointment_id': id,
        'expires_at': a['end_time'],
        'is_locked': a['chat_locked'] == true,
      };
    } else if (options.method == 'GET' && messages != null) {
      body =
          query.keys.any((k) => k.startsWith('after') || k.startsWith('before'))
              ? const <Object>[]
              : _messages();
    } else if (options.method == 'GET') {
      body = switch (path) {
        '/patients/me' => _profile,
        '/patients/me/profiles' => [_profile],
        '/patients/me/appointments' => _appointments(query['status']),
        '/notifications' =>
          query.containsKey('before_id') || query.containsKey('after_id')
              ? const <Object>[]
              : _notifications,
        '/notifications/unread-count' => {
            'unread_count':
                _notifications.where((n) => !n.containsKey('read_at')).length
          },
        _ => const <Object>[],
      };
    } else {
      body = const <String, Object>{};
    }
    return ResponseBody.fromString(jsonEncode({'data': body}), 200, headers: {
      Headers.contentTypeHeader: [Headers.jsonContentType]
    });
  }

  @override
  void close({bool force = false}) {}
}

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
  await store.write('patient_locale_v1', _language);
  final api = ApiClient(
      dio: Dio(BaseOptions(baseUrl: 'http://preview.invalid/api/v1'))
        ..httpClientAdapter = _FixtureAdapter(),
      tokenStore: store);
  final connectivity = ConnectivityService();
  final cache = PatientCacheStore(store);
  final locale = LocaleController(store);
  await locale.load();
  runApp(ListenableBuilder(
    listenable: locale,
    builder: (_, __) => ChangeNotifierProvider<LocaleController>.value(
      value: locale,
      child: MultiBlocProvider(
        providers: [
          BlocProvider(
              create: (_) => ProfileCubit(
                  ProfileRepository(api,
                      cache: cache, connectivity: connectivity),
                  connectivity: connectivity)
                ..load()),
          BlocProvider(
              create: (_) => HomeCubit(
                  HomeRepository(api, cache: cache, connectivity: connectivity),
                  connectivity: connectivity)),
          BlocProvider(
              create: (_) => FamilyProfilesCubit(
                  FamilyProfilesRepository(api,
                      cache: cache, connectivity: connectivity),
                  connectivity: connectivity)
                ..load()),
          BlocProvider(
              create: (_) =>
                  NotificationCubit(NotificationRepository(api))..load()),
        ],
        child: MaterialApp(
          debugShowCheckedModeBanner: false,
          theme: PatientTokens.theme(),
          locale: locale.locale,
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          builder: (_, child) => Banner(
              message: 'PREVIEW DATA',
              location: BannerLocation.bottomStart,
              color: const Color(0xFFB45309),
              child: child ?? const SizedBox()),
          home: Builder(
              builder: (context) => NotificationNavigationHost(
                  cubit: context.read<NotificationCubit>(),
                  child: PatientShell(api: api))),
        ),
      ),
    ),
  ));
}
