import 'dart:math';
import 'dart:convert';
import 'dart:typed_data';
import '../../core/network/client_error_reporter.dart';
import '../../core/network/api_client.dart';
import '../../core/cache/doctor_cache_store.dart';
import '../../core/network/connectivity_service.dart';
import '../../core/video/join_eligibility.dart';
import 'doctor_models.dart';

class CachedDoctorSnapshot {
  const CachedDoctorSnapshot({
    this.queue,
    this.slots,
    this.chats,
    this.questions,
    this.wallet,
    this.duty,
    this.profile,
    this.lastUpdatedAt,
  });
  final List<AppointmentDto>? queue;
  final List<SlotDto>? slots;
  final List<ChatDto>? chats;
  final List<QnaDto>? questions;
  final WalletDto? wallet;
  final bool? duty;
  final ProfileDto? profile;
  final DateTime? lastUpdatedAt;
  bool get hasAny =>
      queue != null ||
      slots != null ||
      chats != null ||
      questions != null ||
      wallet != null ||
      duty != null ||
      profile != null;
}

class DoctorRepository {
  DoctorRepository(this.api, {this.cache, this.connectivity});
  final ApiClient api;
  final DoctorCacheStore? cache;
  final ConnectivityService? connectivity;

  bool get isOffline => connectivity?.isOffline == true;

  void requireOnline() {
    if (isOffline) {
      throw const ApiException(
          null, 'Internet connection is required for this action.');
    }
  }

  Future<CachedResource?> cached(String key) =>
      cache?.read(key) ?? Future<CachedResource?>.value();

  Future<void> cacheWrite(String key, dynamic value, {Duration? ttl}) =>
      cache?.write(key, value, ttl: ttl) ?? Future<void>.value();
  dynamic data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  List<dynamic> listData(dynamic value) {
    if (value is List) return value;
    if (value is Map && value['items'] is List) return value['items'] as List;
    throw const FormatException('Unexpected response shape.');
  }

  Map<String, dynamic> mapData(dynamic value) => value is Map
      ? Map<String, dynamic>.from(value)
      : throw const FormatException('Unexpected response shape.');

  Future<CachedDoctorSnapshot> cachedSnapshot() async {
    final resources = <String, CachedResource?>{};
    for (final key in const [
      'queue',
      'slots',
      'chats',
      'questions',
      'wallet',
      'duty',
      'profile'
    ]) {
      resources[key] = await cached(key);
    }
    List<dynamic>? list(String key) {
      final value = resources[key]?.data;
      return value is List ? value : null;
    }

    List<T>? parse<T>(String key, T Function(Map<String, dynamic>) parser) {
      final values = list(key);
      if (values == null) return null;
      try {
        return values
            .map((e) => parser(Map<String, dynamic>.from(e as Map)))
            .toList();
      } catch (_) {
        // Obsolete or corrupt cache data must never prevent startup.
        return null;
      }
    }

    final queue = parse('queue',
        (j) => AppointmentDto.fromJson({...j, 'queue_actionable': false}));
    final slots = parse('slots', SlotDto.fromJson);
    final chats = parse('chats', ChatDto.fromJson);
    final questions = parse('questions', QnaDto.fromJson);
    final walletData = resources['wallet']?.data;
    final dutyData = resources['duty']?.data;
    final profileData = resources['profile']?.data;
    final dates = resources.values
        .whereType<CachedResource>()
        .map((r) => r.cachedAt)
        .toList();
    dates.sort();
    WalletDto? wallet;
    if (walletData is Map) {
      try {
        wallet = WalletDto.fromJson(Map<String, dynamic>.from(walletData));
      } catch (_) {
        wallet = null;
      }
    }
    ProfileDto? profile;
    if (profileData is Map) {
      try {
        profile = ProfileDto.fromJson(Map<String, dynamic>.from(profileData));
      } catch (_) {
        profile = null;
      }
    }
    return CachedDoctorSnapshot(
      queue: queue,
      slots: slots,
      chats: chats,
      questions: questions,
      wallet: wallet,
      duty: dutyData is Map ? (dutyData['is_on_duty'] == true) : null,
      profile: profile,
      lastUpdatedAt: dates.isEmpty ? null : dates.last,
    );
  }

  Future<ChatDto> conversationForAppointment(AppointmentDto appointment) async {
    requireOnline();
    final raw = mapData(
        data(await api.get('/appointments/${appointment.id}/conversation')));
    return ChatDto.fromJson({
      ...raw,
      'patient_display_name': appointment.patientName,
      'modality': appointment.modality,
      'appointment_status': appointment.status
    });
  }

  Future<List<AppointmentDto>> queue({int limit = 25, int offset = 0}) async {
    try {
      requireOnline();
      final raw = listData(data(await api.get('/doctor/appointments',
          query: {'limit': limit, 'offset': offset, 'view': 'all'})));
      if (offset == 0) {
        await cacheWrite('queue', raw.take(50).toList(),
            ttl: const Duration(minutes: 5));
      }
      return raw
          .map((e) =>
              AppointmentDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Queue', operation: 'load');
      rethrow;
    }
  }

  Future<List<AppointmentDto>> consultationHistory({int offset = 0}) async {
    requireOnline();
    return listData(data(await api.get('/doctor/appointments',
            query: {'limit': 100, 'offset': offset, 'view': 'history'})))
        .map(
            (e) => AppointmentDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  Future<Json> duty() async {
    try {
      requireOnline();
      final raw = mapData(data(await api.get('/doctor/duty')));
      await cacheWrite('duty', raw, ttl: const Duration(minutes: 10));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Queue', operation: 'load');
      rethrow;
    }
  }

  Future<List<AppointmentDto>> allQueue() async {
    final result = <AppointmentDto>[];
    for (var offset = 0; offset < 10000; offset += 100) {
      final batch = await queue(limit: 100, offset: offset);
      result.addAll(batch);
      if (batch.length < 100) return result;
    }
    throw const ApiException(null, 'Consultations exceed display capacity.');
  }

  Future<List<ChatDto>> allChats() async {
    final result = <ChatDto>[];
    for (var offset = 0; offset < 10000; offset += 100) {
      final batch = await chats(limit: 100, offset: offset);
      result.addAll(batch);
      if (batch.length < 100) return result;
    }
    throw const ApiException(null, 'Conversations exceed display capacity.');
  }

  Future<Json> setDuty(bool value) async {
    try {
      requireOnline();
      final raw = mapData(
          data(await api.put('/doctor/duty', data: {'is_on_duty': value})));
      await cacheWrite('duty', raw, ttl: const Duration(minutes: 10));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Queue', operation: 'update');
      rethrow;
    }
  }

  Future<List<SlotDto>> slots(
      {int limit = 25, int offset = 0, DateTime? day}) async {
    try {
      requireOnline();
      final raw = listData(data(await api.get('/doctor/slots', query: {
        'limit': limit,
        'offset': offset,
        if (day != null)
          'day':
              '${day.year.toString().padLeft(4, '0')}-${day.month.toString().padLeft(2, '0')}-${day.day.toString().padLeft(2, '0')}'
      })));
      if (offset == 0) {
        await cacheWrite('slots', raw.take(100).toList(),
            ttl: const Duration(minutes: 10));
      }
      return raw
          .map((e) => SlotDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Schedule', operation: 'load');
      rethrow;
    }
  }

  Future<List<SlotDto>> allSlots({DateTime? day}) async {
    final result = <SlotDto>[];
    for (var offset = 0; offset < 10000; offset += 100) {
      final batch = await slots(limit: 100, offset: offset, day: day);
      result.addAll(batch);
      if (batch.length < 100) return result;
    }
    throw const ApiException(null, 'Schedule exceeds display capacity.');
  }

  Future<Json> createSlot(DateTime start, DateTime end) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/doctor/slots',
          data: {
            'start_time': start.toUtc().toIso8601String(),
            'end_time': end.toUtc().toIso8601String()
          },
          idempotent: true)));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Schedule', operation: 'create_slot');
      rethrow;
    }
  }

  Future<Json> cancelSlot(String id) async {
    try {
      requireOnline();
      return mapData(data(await api.delete('/doctor/slots/$id')));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Schedule', operation: 'cancel_slot');
      rethrow;
    }
  }

  /// [status] is `"AVAILABLE"` or `"BLOCKED"`. A 409 means the slot is no
  /// longer in the expected prior state (e.g. it was just booked) and
  /// surfaces to the caller as an [ApiException], same as [cancelSlot].
  Future<Json> updateSlotStatus(String id, String status) async {
    try {
      requireOnline();
      return mapData(
          data(await api.patch('/doctor/slots/$id', data: {'status': status})));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Schedule', operation: 'update_slot_status');
      rethrow;
    }
  }

  Future<List<ChatDto>> chats({int limit = 25, int offset = 0}) async {
    try {
      requireOnline();
      final raw = listData(data(await api
          .get('/doctor/chats', query: {'limit': limit, 'offset': offset})));
      if (offset == 0) {
        await cacheWrite('chats', raw.take(50).toList(),
            ttl: const Duration(minutes: 10));
      }
      return raw
          .map((e) => ChatDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'load');
      rethrow;
    }
  }

  Future<MessageDto> uploadClinicalImage(String conversationId, Uint8List bytes,
      String mime, String requestKey) async {
    requireOnline();
    final raw = data(await api.post('/chat/$conversationId/attachments',
        idempotencyKey: requestKey,
        data: {'mime_type': mime, 'base64_content': base64Encode(bytes)}));
    return MessageDto.fromJson(mapData(raw));
  }

  Future<Uint8List> clinicalImage(
      String conversationId, String messageId) async {
    requireOnline();
    final raw = mapData(data(
        await api.get('/chat/$conversationId/messages/$messageId/attachment')));
    return base64Decode(raw['base64_content'] as String);
  }

  Future<void> acknowledgeConsultation(String appointmentId) async {
    requireOnline();
    await api.post('/appointments/$appointmentId/trace-ack',
        data: {'event': 'CONSULTATION_OPENED'});
  }

  final _pendingMessages = <String, ({String content, String key})>{};
  Future<void> acknowledgeChat(ChatDto chat, String event,
      {List<String> messages = const [], DateTime? clientOccurredAt}) async {
    requireOnline();
    if (chat.appointmentId == null) return;
    await api.post('/appointments/${chat.appointmentId}/trace-ack', data: {
      'event': event,
      'conversation_id': chat.id,
      'message_ids': messages,
      if (clientOccurredAt != null)
        'client_occurred_at': clientOccurredAt.toUtc().toIso8601String(),
    });
  }

  Future<List<MessageDto>> newerMessages(
      String conversation, MessageDto after) async {
    requireOnline();
    final raw = listData(data(await api.get('/chat/$conversation/messages',
        query: {'after_time': after.sentAt, 'after_id': after.id})));
    return raw
        .map((e) => MessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  Future<List<MessageDto>> olderMessages(
      String conversation, MessageDto before) async {
    requireOnline();
    final raw = listData(data(await api.get('/chat/$conversation/messages',
        query: {'before_time': before.sentAt, 'before_id': before.id})));
    return raw
        .map((e) => MessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
        .toList();
  }

  Future<List<MessageDto>> messages(String id) async {
    try {
      final cachedMessages = await cached('chat_messages');
      if (isOffline && cachedMessages?.data is Map) {
        try {
          final raw = (cachedMessages!.data[id] as List?) ?? const [];
          return raw
              .map((e) =>
                  MessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
              .toList();
        } catch (_) {
          // Fall through to the authoritative request/error path.
        }
      }
      requireOnline();
      final raw = listData(data(await api.get('/chat/$id/messages')));
      final all = <String, dynamic>{};
      if (cachedMessages?.data is Map) {
        all.addAll(Map<String, dynamic>.from(cachedMessages!.data as Map));
      }
      all[id] = raw;
      while (all.length > 20) {
        all.remove(all.keys.first);
      }
      await cacheWrite('chat_messages', all, ttl: const Duration(hours: 24));
      return raw
          .map((e) => MessageDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'load');
      rethrow;
    }
  }

  Future<Json> sendMessage(String id, String content) async {
    try {
      requireOnline();
      if (_pendingMessages[id]?.content != content) {
        final key = List.generate(
            16,
            (_) => Random.secure()
                .nextInt(256)
                .toRadixString(16)
                .padLeft(2, '0')).join();
        _pendingMessages[id] = (content: content, key: key);
      }
      final message = mapData(data(await api.post('/chat/$id/messages',
          data: {'content': content},
          idempotencyKey: _pendingMessages[id]!.key)));
      _pendingMessages.remove(id);
      return message;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Chat', operation: 'send_message');
      rethrow;
    }
  }

  Future<List<Json>> history(String appointment,
      {int limit = 25, int offset = 0}) async {
    try {
      requireOnline();
      return listData(data(await api.get(
              '/doctor/appointments/$appointment/patient-history',
              query: {'limit': limit, 'offset': offset})))
          .map((e) => Map<String, dynamic>.from(e as Map))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Patients', operation: 'load');
      rethrow;
    }
  }

  Future<Json> historyDetail(String appointment, String episode) async {
    try {
      requireOnline();
      return mapData(data(await api
          .get('/doctor/appointments/$appointment/patient-history/$episode')));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Patients', operation: 'load');
      rethrow;
    }
  }

  Future<List<QnaDto>> questions({int limit = 25, int offset = 0}) async {
    try {
      if (isOffline) {
        final saved = await cached('questions');
        if (saved?.data is List) {
          try {
            return (saved!.data as List)
                .map(
                    (e) => QnaDto.fromJson(Map<String, dynamic>.from(e as Map)))
                .toList();
          } catch (_) {
            // Ignore invalid cached DTOs and keep the backend authoritative.
          }
        }
      }
      requireOnline();
      final raw = listData(data(await api.get('/doctor/qna/questions',
          query: {'limit': limit, 'offset': offset, 'status': 'ALL'})));
      await cacheWrite('questions', raw.take(50).toList(),
          ttl: const Duration(minutes: 15));
      return raw
          .map((e) => QnaDto.fromJson(Map<String, dynamic>.from(e as Map)))
          .toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Qna', operation: 'load');
      rethrow;
    }
  }

  Future<Json> answer(String id, String answerText) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/doctor/qna/questions/$id/answers',
          data: {'answer_text': answerText})));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Qna', operation: 'update');
      rethrow;
    }
  }

  Future<ProfileDto?> cachedProfile() async {
    final saved = await cached('profile');
    if (saved?.data is Map) {
      try {
        return ProfileDto.fromJson(
            Map<String, dynamic>.from(saved!.data as Map));
      } catch (_) {}
    }
    return null;
  }

  Future<ProfileDto> profile() async {
    try {
      requireOnline();
      final raw = mapData(data(await api.get('/doctor/profile')));
      await cacheWrite('profile', raw, ttl: const Duration(hours: 12));
      return ProfileDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Settings', operation: 'load');
      rethrow;
    }
  }

  /// `GET /config/booking-policy` — the server-authoritative financial-
  /// module mode. Fails safe to `paymentRequired: true` (the full-earnings
  /// UI) on any error, since hiding real financial actions in enabled mode
  /// is worse than briefly showing them during a transient failure.
  Future<BookingPolicyDto> bookingPolicy() async {
    try {
      requireOnline();
      final raw = mapData(data(await api.get('/config/booking-policy')));
      return BookingPolicyDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'load_booking_policy');
      rethrow;
    }
  }

  Future<Json> updateProfile(String name) async {
    try {
      requireOnline();
      return mapData(
          data(await api.patch('/doctor/profile', data: {'full_name': name})));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Settings', operation: 'update');
      rethrow;
    }
  }

  /// Compression/resizing and the 40KB ceiling are entirely server-side
  /// (`UploadSecurityService`) — this client sends the picked image's bytes
  /// unmodified, the same "server is the sole source of truth on size"
  /// choice already made for Rx-photo uploads.
  Future<void> uploadPhoto({
    required Uint8List bytes,
    required String mimeType,
  }) async {
    try {
      requireOnline();
      await api.post('/doctor/profile/photo', data: {
        'mime_type': mimeType,
        'base64_content': base64Encode(bytes),
      });
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Settings', operation: 'upload_photo');
      rethrow;
    }
  }

  Future<void> deletePhoto() async {
    try {
      requireOnline();
      await api.delete('/doctor/profile/photo');
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Settings', operation: 'delete_photo');
      rethrow;
    }
  }

  /// Returns `null` when no photo is set (a 404 from the backend) rather
  /// than throwing — "no photo yet" is an expected, common state for this
  /// optional feature, not an error condition the UI needs to surface.
  Future<Uint8List?> fetchPhotoBytes() async {
    try {
      requireOnline();
      final bytes = await api.getBytes('/doctor/profile/photo');
      return Uint8List.fromList(bytes);
    } on ApiException catch (error, stack) {
      if (error.status == 404) return null;
      ClientErrorReporter.observe(error, stack,
          screen: 'Settings', operation: 'fetch_photo');
      rethrow;
    }
  }

  Future<WalletDto?> cachedWallet() async {
    final saved = await cached('wallet');
    if (saved?.data is Map) {
      try {
        return WalletDto.fromJson(
            Map<String, dynamic>.from(saved!.data as Map));
      } catch (_) {}
    }
    return null;
  }

  Future<WalletDto> wallet() async {
    try {
      requireOnline();
      final raw = mapData(data(await api.get('/doctor/wallet/summary')));
      await cacheWrite('wallet', raw, ttl: const Duration(minutes: 15));
      return WalletDto.fromJson(raw);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'load');
      rethrow;
    }
  }

  Future<Json> payoutMethod() async {
    try {
      requireOnline();
      final methods =
          listData(data(await api.get('/doctor/wallet/payout-methods')))
              .map(mapData)
              .toList();
      final primary = methods
          .where(
              (m) => m['is_primary'] == true && m['binding_status'] == 'ACTIVE')
          .firstOrNull;
      final raw = <String, dynamic>{
        'configured': primary != null,
        if (primary != null) ...primary,
        if (primary != null)
          'display_label':
              '${primary['method_type']} ${primary['masked_destination']}',
        'methods': methods,
      };
      await cacheWrite('payout_method', raw, ttl: const Duration(hours: 12));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'load');
      rethrow;
    }
  }

  Future<Json> devBindPayoutMethod() async {
    requireOnline();
    final raw = mapData(data(await api.post(
        '/doctor/wallet/payout-methods/dev-bind',
        data: {'method_type': 'MPESA'})));
    await cacheWrite('payout_method', raw, ttl: const Duration(hours: 12));
    return raw;
  }

  Future<Json?> cachedPayoutMethod() async {
    final saved = await cached('payout_method');
    if (saved?.data is Map) {
      try {
        return Map<String, dynamic>.from(saved!.data as Map);
      } catch (_) {}
    }
    return null;
  }

  Future<Json> setPayoutMethod(
      String methodType, String accountIdentifier) async {
    try {
      requireOnline();
      final raw = mapData(data(await api.put('/doctor/wallet/payout-method',
          data: {
            'method_type': methodType,
            'account_identifier': accountIdentifier
          })));
      await cacheWrite('payout_method', raw, ttl: const Duration(hours: 12));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'update');
      rethrow;
    }
  }

  /// `POST /doctor/wallet/payout-methods/bind` — the real, reachable-in-
  /// every-environment provider binding flow (`wallet_handlers::bind_payout_destination`),
  /// writing to `doctor_payout_destinations` (the table Admin's payout
  /// routing and the disbursement pipeline actually read from), unlike
  /// [setPayoutMethod]'s legacy `doctor_payout_method` pair above. The
  /// backend hard-stops (409) if the platform has no active M-Pesa provider
  /// configured yet — surfaced honestly here, not silently retried or
  /// swallowed.
  Future<Json> bindPayoutDestination(String accountIdentifier,
      {bool? makePrimary}) async {
    try {
      requireOnline();
      final raw = mapData(data(await api
          .post('/doctor/wallet/payout-methods/bind', idempotent: true, data: {
        'method_type': 'MPESA',
        'account_identifier': accountIdentifier,
        if (makePrimary != null) 'make_primary': makePrimary,
      })));
      await cacheWrite('payout_method', raw, ttl: const Duration(hours: 12));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'bind_payout_destination');
      rethrow;
    }
  }

  /// `POST /doctor/wallet/payout-methods/set-primary`.
  Future<void> setPrimaryPayoutDestination(String destinationId) async {
    try {
      requireOnline();
      await api.post('/doctor/wallet/payout-methods/set-primary',
          idempotent: true, data: {'destination_id': destinationId});
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'set_primary_payout_destination');
      rethrow;
    }
  }

  Future<List<Json>> ledger({int limit = 25, int offset = 0}) async {
    try {
      if (isOffline) {
        final saved = await cached('ledger');
        if (saved?.data is List) {
          return (saved!.data as List)
              .map((e) => Map<String, dynamic>.from(e as Map))
              .toList();
        }
      }
      requireOnline();
      final raw = listData(data(await api.get('/doctor/wallet/ledger',
          query: {'limit': limit, 'offset': offset})));
      await cacheWrite('ledger', raw.take(100).toList(),
          ttl: const Duration(minutes: 30));
      return raw.map((e) => Map<String, dynamic>.from(e as Map)).toList();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'load');
      rethrow;
    }
  }

  Future<Json> monthlyStatement(String month) async {
    try {
      requireOnline();
      final raw = mapData(data(
          await api.get('/doctor/wallet/statements', query: {'month': month})));
      await cacheWrite('statement_$month', raw,
          ttl: const Duration(minutes: 20));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Wallet', operation: 'load');
      rethrow;
    }
  }

  Future<Json?> cachedMonthlyStatement(String month) async {
    final saved = await cached('statement_$month');
    if (saved?.data is Map) {
      return Map<String, dynamic>.from(saved!.data as Map);
    }
    return null;
  }

  Future<List<Json>?> cachedLedger() async {
    final saved = await cached('ledger');
    if (saved?.data is List) {
      try {
        return (saved!.data as List)
            .map((e) => Map<String, dynamic>.from(e as Map))
            .toList();
      } catch (_) {
        // Invalid cached ledger data is discarded by the caller's refresh.
      }
    }
    return null;
  }

  Future<Json?> cachedSummary(String id) async {
    final saved = await cached('summaries');
    if (saved?.data is Map && (saved!.data as Map)[id] is Map) {
      try {
        return _safeSummary(
            Map<String, dynamic>.from((saved.data as Map)[id] as Map));
      } catch (_) {}
    }
    return null;
  }

  Future<Json> summary(String id) async {
    try {
      requireOnline();
      final raw =
          mapData(data(await api.get('/doctor/consultations/$id/summary')));
      final saved = await cached('summaries');
      final summaries = <String, dynamic>{};
      if (saved?.data is Map) {
        summaries.addAll(Map<String, dynamic>.from(saved!.data as Map));
      }
      // Clinical intake requires fresh authorization and is never an offline grant.
      summaries[id] = _safeSummary(raw);
      await cacheWrite('summaries', summaries,
          ttl: const Duration(minutes: 10));
      return raw;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'load_consultation');
      rethrow;
    }
  }

  static Json _safeSummary(Json raw) => {
        for (final key in const [
          'appointment_id',
          'patient_id',
          'status',
          'completed_at',
          'consultation_id',
          'started_at',
          'ended_at'
        ])
          if (raw.containsKey(key)) key: raw[key],
      };

  Future<Json> complete(String id, String outcome) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/consultations/$id/complete',
          data: {'outcome': outcome}, idempotent: true)));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'complete_consultation');
      rethrow;
    }
  }

  Future<Json> completeNoRx(String id, String reason) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/consultations/$id/complete-no-rx',
          data: {'reason': reason}, idempotent: true)));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'complete_consultation');
      rethrow;
    }
  }

  /// `GET /consultations/:id/join-eligibility` — the server's authoritative,
  /// read-only answer to "may I enter this room now?" (same policy and
  /// authorization as the token mint; issues no token, creates no session).
  Future<JoinEligibility> joinEligibility(String id) async {
    try {
      requireOnline();
      final raw = data(await api.get('/consultations/$id/join-eligibility'));
      final parsed = JoinEligibility.tryParse(raw);
      if (parsed == null) {
        throw const FormatException('Unexpected response shape.');
      }
      return parsed;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Telehealth', operation: 'join_eligibility');
      rethrow;
    }
  }

  Future<Json> rtcToken(String id) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/consultations/$id/rtc-token')));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Telehealth', operation: 'mint_rtc_token');
      rethrow;
    }
  }

  /// Best-effort by design (mirrors the backend's own
  /// `record_telemetry_events` fail-open stance: "ingesting observability
  /// data must never be able to break an active consultation") — swallows
  /// failures rather than rethrowing, since every call site fires this
  /// without awaiting into a user-visible error path.
  Future<void> telemetry(String id, List<Map<String, dynamic>> events) async {
    try {
      requireOnline();
      await api.post('/consultations/$id/telemetry/events',
          data: {'events': events});
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Telehealth', operation: 'call_telemetry');
    }
  }

  Future<dynamic> uploadPrescription(
      String id, String base64Photo, String? notes,
      {String? mimeType, void Function(int, int)? onSendProgress}) async {
    try {
      requireOnline();
      return mapData(data(await api.post('/consultations/$id/prescription',
          data: {
            'base64_photo': base64Photo,
            'doctor_notes': notes,
            if (mimeType != null) 'mime_type': mimeType
          },
          onSendProgress: onSendProgress,
          idempotent: true)));
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'upload_prescription');
      rethrow;
    }
  }

  Future<String> documentUrl(String appointmentId, String documentId,
      {String? currentAppointmentId}) async {
    try {
      requireOnline();
      final url = textOf(mapData(data(await api
          .get('/appointments/$appointmentId/documents/$documentId', query: {
        if (currentAppointmentId != null)
          'current_appointment_id': currentAppointmentId
      })))['download_url']);
      return url.isEmpty ? '' : api.resolveServerUrl(url).toString();
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'load_document');
      rethrow;
    }
  }

  /// Remediation E2: mints a fresh, single-use document-download grant via
  /// [documentUrl] and immediately redeems it for the actual bytes, in-memory
  /// — never via `launchUrl`/an external application. The grant is one-time
  /// (server-side `GETDEL`), so this must be called again for every retry
  /// rather than reusing a previously-resolved URL.
  Future<TypedBytes> fetchDocumentBytes(String appointmentId, String documentId,
      {String? currentAppointmentId}) async {
    final url = await documentUrl(appointmentId, documentId,
        currentAppointmentId: currentAppointmentId);
    if (url.isEmpty) {
      throw const ApiException(404, 'This document is unavailable.');
    }
    try {
      return await api.getBytesWithType(url);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'SmartRx', operation: 'fetch_document_bytes');
      rethrow;
    }
  }
}
