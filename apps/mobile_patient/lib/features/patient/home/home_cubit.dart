import 'dart:async';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import 'home_models.dart';
import 'home_repository.dart';

const _unset = Object();

class HomeState {
  const HomeState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.upcoming = const [],
    this.lastUpdatedAt,
  });
  final bool loading, refreshing, offline;
  final String? error;
  final List<AppointmentSummaryDto> upcoming;
  final DateTime? lastUpdatedAt;

  /// The single nearest upcoming confirmed/pending consultation, if any —
  /// the prototype's Home `.consult-card` (`view-0`) only ever shows one.
  AppointmentSummaryDto? get nextUpcoming =>
      upcoming.isEmpty ? null : upcoming.first;

  HomeState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    List<AppointmentSummaryDto>? upcoming,
    DateTime? lastUpdatedAt,
  }) =>
      HomeState(
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        offline: offline ?? this.offline,
        error: identical(error, _unset) ? this.error : error as String?,
        upcoming: upcoming ?? this.upcoming,
        lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
      );
}

class HomeCubit extends Cubit<HomeState> {
  HomeCubit(this.repo, {this.connectivity}) : super(const HomeState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        load();
      }
    });
  }
  final HomeRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;
  // `null` = the caller's own self profile. Set via [setActiveProfile], kept
  // across calls to [load] (e.g. pull-to-refresh) so a refresh doesn't
  // silently snap back to "self" while a dependent is active.
  String? _activeProfileId;

  /// Re-queries this Home card for a different profile (§5b — the Family
  /// Profiles switcher). A no-op if the id hasn't actually changed, so a
  /// rebuild that just re-reads the same active profile doesn't trigger a
  /// redundant network call.
  void setActiveProfile(String? profileId) {
    if (profileId == _activeProfileId) return;
    _activeProfileId = profileId;
    load();
  }

  Future<void> load() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final viewingSelf = _activeProfileId == null;
    final List<AppointmentSummaryDto> cached = viewingSelf
        ? await repo.cachedUpcomingAppointments()
        : const <AppointmentSummaryDto>[];
    if (isClosed) return;
    if (cached.isNotEmpty) {
      emit(state.copyWith(
          loading: false,
          refreshing: true,
          offline: false,
          error: null,
          upcoming: cached));
    } else {
      emit(state.copyWith(
          loading: true,
          refreshing: false,
          offline: false,
          error: null,
          upcoming: const []));
    }
    try {
      final upcoming =
          await repo.upcomingAppointments(profileId: _activeProfileId);
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        upcoming: upcoming,
        lastUpdatedAt: DateTime.now().toUtc(),
      ));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: cached.isNotEmpty && e is ApiException && e.status == null,
        error: e is ApiException
            ? e.userMessage
            : 'Unable to load your upcoming consultations.',
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  @override
  Future<void> close() {
    _connectivitySubscription?.cancel();
    return super.close();
  }
}
