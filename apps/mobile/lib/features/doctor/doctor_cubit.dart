import 'dart:async';
import 'dart:typed_data';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/time/kenya_time.dart';
import '../../core/network/connectivity_service.dart';
import 'doctor_repository.dart';
import 'doctor_models.dart';

const _unset = Object();

class DoctorState {
  const DoctorState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.lastUpdatedAt,
    this.queue = const [],
    this.slots = const [],
    this.chats = const [],
    this.hasMoreChats = false,
    this.loadingMoreChats = false,
    this.questions = const [],
    this.wallet,
    this.duty = false,
    this.dutyUpdating = false,
    this.profile,
    this.photoLoading = false,
    this.photoSaving = false,
    this.photoBytes,
    this.photoError,
  });
  final bool loading, refreshing, offline, duty, dutyUpdating;
  final bool hasMoreChats, loadingMoreChats;
  final String? error;
  final DateTime? lastUpdatedAt;
  final List<dynamic> queue, slots, chats, questions;
  final dynamic wallet;
  final ProfileDto? profile;

  /// Photo state is deliberately independent of [error] (the rest of the
  /// dashboard/profile data) — a failed photo upload must never be mistaken
  /// for, or block, anything else this cubit loads.
  final bool photoLoading, photoSaving;
  final Uint8List? photoBytes;
  final String? photoError;

  DoctorState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    DateTime? lastUpdatedAt,
    List<dynamic>? queue,
    List<dynamic>? slots,
    List<dynamic>? chats,
    bool? hasMoreChats,
    bool? loadingMoreChats,
    List<dynamic>? questions,
    Object? wallet = _unset,
    bool? duty,
    bool? dutyUpdating,
    Object? profile = _unset,
    bool? photoLoading,
    bool? photoSaving,
    Object? photoBytes = _unset,
    Object? photoError = _unset,
  }) {
    return DoctorState(
      loading: loading ?? this.loading,
      refreshing: refreshing ?? this.refreshing,
      offline: offline ?? this.offline,
      error: identical(error, _unset) ? this.error : error as String?,
      lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
      queue: queue ?? this.queue,
      slots: slots ?? this.slots,
      chats: chats ?? this.chats,
      hasMoreChats: hasMoreChats ?? this.hasMoreChats,
      loadingMoreChats: loadingMoreChats ?? this.loadingMoreChats,
      questions: questions ?? this.questions,
      wallet: identical(wallet, _unset) ? this.wallet : wallet,
      duty: duty ?? this.duty,
      dutyUpdating: dutyUpdating ?? this.dutyUpdating,
      profile:
          identical(profile, _unset) ? this.profile : profile as ProfileDto?,
      photoLoading: photoLoading ?? this.photoLoading,
      photoSaving: photoSaving ?? this.photoSaving,
      photoBytes: identical(photoBytes, _unset)
          ? this.photoBytes
          : photoBytes as Uint8List?,
      photoError:
          identical(photoError, _unset) ? this.photoError : photoError as String?,
    );
  }
}

class DoctorCubit extends Cubit<DoctorState> {
  DoctorCubit(this.repo, {this.connectivity}) : super(const DoctorState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        emit(state.copyWith(offline: false));
      }
    });
  }
  final DoctorRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;

  bool get isOffline => connectivity?.isOffline ?? repo.isOffline;

  void _applyCached(CachedDoctorSnapshot cached, {bool refreshing = true}) {
    emit(state.copyWith(
      loading: false,
      refreshing: refreshing,
      offline: false,
      error: null,
      lastUpdatedAt: cached.lastUpdatedAt,
      queue: cached.queue,
      slots: cached.slots,
      chats: cached.chats,
      questions: cached.questions,
      wallet: cached.wallet,
      duty: cached.duty,
      profile: cached.profile,
    ));
  }

  Future<void> loadQueue() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final cached = await repo.cachedSnapshot();
    if (isClosed) return;
    if (cached.queue != null || cached.duty != null) {
      emit(state.copyWith(
          queue: cached.queue ?? state.queue,
          duty: cached.duty ?? state.duty,
          loading: false,
          refreshing: true,
          offline: false,
          error: null));
    } else {
      emit(state.copyWith(
          loading: true, refreshing: false, offline: false, error: null));
    }
    try {
      final results = await Future.wait([repo.allQueue(), repo.duty()]);
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        lastUpdatedAt: DateTime.now().toUtc(),
        queue: List<dynamic>.from(results[0] as List<AppointmentDto>),
        duty: (results[1] as Json)['is_on_duty'] == true,
      ));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline:
            (cached.queue != null || cached.duty != null) && _isOfflineError(e),
        error: _message(e, 'Unable to load the doctor queue.'),
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  Future<void> loadAll() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final cached = await repo.cachedSnapshot();
    if (isClosed) return;
    if (cached.hasAny) {
      _applyCached(cached);
    } else {
      emit(state.copyWith(
          loading: true, refreshing: false, offline: false, error: null));
    }
    try {
      // A non-approved Doctor must still see their real review state, even
      // though clinical endpoints correctly return 403 for them.
      final profile = await repo.profile();
      if (isClosed) return;
      if (!profile.verified) {
        emit(DoctorState(
            profile: profile, lastUpdatedAt: DateTime.now().toUtc()));
        return;
      }
      final r = await Future.wait([
        repo.allQueue(),
        repo.allSlots(day: kenyaTime(DateTime.now())),
        repo.chats(limit: 100),
        repo.questions(),
        repo.wallet(),
        repo.duty(),
      ]);
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        lastUpdatedAt: DateTime.now().toUtc(),
        queue: List<dynamic>.from(r[0] as List<AppointmentDto>),
        slots: List<dynamic>.from(r[1] as List<SlotDto>),
        chats: List<dynamic>.from(r[2] as List<ChatDto>),
        hasMoreChats: (r[2] as List<ChatDto>).length == 100,
        questions: List<dynamic>.from(r[3] as List<QnaDto>),
        wallet: r[4],
        duty: (r[5] as Json)['is_on_duty'] == true,
        profile: profile,
      ));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: cached.hasAny && _isOfflineError(e),
        error: _message(
            e, 'Unable to load Doctor data. Check your connection and retry.'),
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  final Map<String, Future<void>> _scheduleReads = {};
  bool _chatListInFlight = false;
  int _chatPageDepth = 1;
  bool _questionsInFlight = false;

  /// Q&A refresh must not depend on unrelated wallet/queue requests. In
  /// particular, a concurrent-answer conflict needs authoritative Q&A data.
  Future<void> refreshQuestions() async {
    if (isClosed || _questionsInFlight) return;
    _questionsInFlight = true;
    emit(state.copyWith(refreshing: true, error: null));
    try {
      final questions = await repo.questions();
      if (!isClosed) {
        emit(state.copyWith(
            questions: questions,
            loading: false,
            refreshing: false,
            offline: isOffline,
            error: null,
            lastUpdatedAt: DateTime.now().toUtc()));
      }
    } catch (error) {
      if (!isClosed) {
        emit(state.copyWith(
            loading: false,
            refreshing: false,
            offline: _isOfflineError(error),
            error: 'Questions could not be refreshed. Please retry.'));
      }
    } finally {
      _questionsInFlight = false;
    }
  }

  Future<void> refreshChats({bool older = false}) async {
    if (isClosed || _chatListInFlight || isOffline) return;
    _chatListInFlight = true;
    if (older) emit(state.copyWith(loadingMoreChats: true));
    try {
      final depth = older ? _chatPageDepth + 1 : _chatPageDepth;
      final rows = older ? state.chats.cast<ChatDto>().toList() : <ChatDto>[];
      var more = false;
      for (var page = older ? _chatPageDepth : 0; page < depth; page++) {
        final batch = await repo.chats(limit: 100, offset: page * 100);
        rows.addAll(batch);
        more = batch.length == 100;
        if (!more) break;
      }
      if (!isClosed) {
        _chatPageDepth = (rows.length / 100).ceil().clamp(1, depth);
        emit(state.copyWith(
            chats: rows,
            hasMoreChats: more,
            loadingMoreChats: false,
            error: null));
      }
    } catch (error) {
      if (!isClosed) {
        emit(state.copyWith(
            loadingMoreChats: false,
            error: _message(
                error, 'Conversations could not be refreshed. Please retry.')));
      }
    } finally {
      _chatListInFlight = false;
    }
  }

  Future<void> refreshSlots({DateTime? day}) {
    if (isClosed || isOffline) return Future.value();
    final selected = day ?? kenyaTime(DateTime.now());
    final key = '${selected.year}-${selected.month}-${selected.day}';
    return _scheduleReads.putIfAbsent(
        key,
        () => _refreshSlotDay(selected).whenComplete(() {
              _scheduleReads.remove(key);
            }));
  }

  Future<void> _refreshSlotDay(DateTime day) async {
    try {
      final slots = await repo.allSlots(day: day);
      if (!isClosed) {
        final ids = slots.map((s) => s.id).toSet();
        final otherDays = state.slots.cast<SlotDto>().where((s) {
          if (ids.contains(s.id) || s.start == null) return false;
          final date = kenyaTime(s.start!);
          return date.year != day.year ||
              date.month != day.month ||
              date.day != day.day;
        });
        emit(state.copyWith(
            slots: [...otherDays, ...slots], error: null, offline: false));
      }
    } catch (error) {
      if (!isClosed) {
        emit(state.copyWith(
            error: _message(error,
                'Schedule could not be refreshed. Showing last loaded availability.'),
            offline: _isOfflineError(error)));
      }
    }
  }

  Future<void> toggleDuty() async {
    if (state.dutyUpdating) return;
    if (isOffline) {
      emit(state.copyWith(
          error: 'Internet connection is required to change duty status.',
          offline: true));
      return;
    }
    final old = state.duty;
    emit(state.copyWith(dutyUpdating: true, error: null));
    try {
      final d = await repo.setDuty(!old);
      if (isClosed) return;
      emit(state.copyWith(
          duty: d['is_on_duty'] == true,
          dutyUpdating: false,
          offline: false,
          error: null,
          lastUpdatedAt: DateTime.now().toUtc()));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        duty: old,
        dutyUpdating: false,
        error: _message(e, 'Duty status could not be updated.'),
      ));
    }
  }

  static String _message(Object error, String fallback) {
    if (error is ApiException) return error.userMessage;
    return fallback;
  }

  /// Loads (or reloads, e.g. after an upload/removal) the caller's own
  /// photo bytes. Silently leaves [DoctorState.photoBytes] at whatever it
  /// already was on failure — a transient failure to *load* the current
  /// avatar is not worth surfacing as a blocking error.
  Future<void> loadPhoto() async {
    emit(state.copyWith(photoLoading: true));
    try {
      final bytes = await repo.fetchPhotoBytes();
      if (isClosed) return;
      emit(state.copyWith(photoLoading: false, photoBytes: bytes));
    } catch (_) {
      if (isClosed) return;
      emit(state.copyWith(photoLoading: false));
    }
  }

  /// Returns true on success. Upload failure is reported via
  /// [DoctorState.photoError] only — it must never affect [error], since a
  /// profile picture is optional and its failure must never block anything
  /// else this cubit owns.
  Future<bool> uploadPhoto(Uint8List bytes, String mimeType) async {
    emit(state.copyWith(photoSaving: true, photoError: null));
    try {
      await repo.uploadPhoto(bytes: bytes, mimeType: mimeType);
      final refreshed = await repo.fetchPhotoBytes();
      if (isClosed) return true;
      emit(state.copyWith(photoSaving: false, photoBytes: refreshed));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
          photoSaving: false,
          photoError: _message(e, 'Unable to upload your photo.')));
      return false;
    }
  }

  Future<bool> removePhoto() async {
    emit(state.copyWith(photoSaving: true, photoError: null));
    try {
      await repo.deletePhoto();
      if (isClosed) return true;
      emit(state.copyWith(photoSaving: false, photoBytes: null));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
          photoSaving: false,
          photoError: _message(e, 'Unable to remove your photo.')));
      return false;
    }
  }

  bool _isOfflineError(Object error) =>
      repo.isOffline || (error is ApiException && error.status == null);

  @override
  Future<void> close() async {
    await _connectivitySubscription?.cancel();
    return super.close();
  }
}
