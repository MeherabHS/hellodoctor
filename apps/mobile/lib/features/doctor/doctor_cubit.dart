import 'dart:async';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
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
    this.questions = const [],
    this.wallet,
    this.duty = false,
    this.dutyUpdating = false,
    this.profile,
  });
  final bool loading, refreshing, offline, duty, dutyUpdating;
  final String? error;
  final DateTime? lastUpdatedAt;
  final List<dynamic> queue, slots, chats, questions;
  final dynamic wallet;
  final ProfileDto? profile;

  DoctorState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    DateTime? lastUpdatedAt,
    List<dynamic>? queue,
    List<dynamic>? slots,
    List<dynamic>? chats,
    List<dynamic>? questions,
    Object? wallet = _unset,
    bool? duty,
    bool? dutyUpdating,
    Object? profile = _unset,
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
      questions: questions ?? this.questions,
      wallet: identical(wallet, _unset) ? this.wallet : wallet,
      duty: duty ?? this.duty,
      dutyUpdating: dutyUpdating ?? this.dutyUpdating,
      profile:
          identical(profile, _unset) ? this.profile : profile as ProfileDto?,
    );
  }
}

class DoctorCubit extends Cubit<DoctorState> {
  DoctorCubit(this.repo, {this.connectivity}) : super(const DoctorState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        loadAll();
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
      _applyCached(cached);
    } else {
      emit(state.copyWith(
          loading: true, refreshing: false, offline: false, error: null));
    }
    try {
      final results = await Future.wait([repo.queue(), repo.duty()]);
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
        repo.queue(),
        repo.allSlots(),
        repo.chats(),
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

  bool _scheduleInFlight = false;
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

  Future<void> refreshSlots() async {
    if (isClosed || _scheduleInFlight || _refreshInFlight || isOffline) return;
    _scheduleInFlight = true;
    try {
      final slots = await repo.allSlots();
      if (!isClosed) {
        emit(state.copyWith(slots: slots, error: null, offline: false));
      }
    } catch (error) {
      if (!isClosed) {
        emit(state.copyWith(
            error: _message(error,
                'Schedule could not be refreshed. Showing last loaded availability.'),
            offline: _isOfflineError(error)));
      }
    } finally {
      _scheduleInFlight = false;
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

  bool _isOfflineError(Object error) =>
      repo.isOffline || (error is ApiException && error.status == null);

  @override
  Future<void> close() async {
    await _connectivitySubscription?.cancel();
    return super.close();
  }
}
