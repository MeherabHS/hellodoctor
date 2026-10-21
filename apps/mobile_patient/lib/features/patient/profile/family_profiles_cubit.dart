import 'dart:async';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import 'family_profiles_repository.dart';
import 'profile_models.dart';

const _unset = Object();

class FamilyProfilesState {
  const FamilyProfilesState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.profiles = const [],
    this.activeProfileId,
    this.creating = false,
    this.createError,
  });
  final bool loading, refreshing, offline, creating;
  final String? error;
  final List<PatientProfileDto> profiles;
  // `null` means "the caller's own self profile" — the same default the
  // backend's `acting_patient_profile_id` uses when `profile_id` is omitted,
  // so an app that never touches the switcher behaves exactly as before.
  final String? activeProfileId;
  final String? createError;

  /// Resolves the active profile's own DTO, falling back to whichever
  /// profile is actually flagged `isSelf` (not just "the first one") in case
  /// the list ever returns dependents before the self row.
  PatientProfileDto? get activeProfile {
    if (profiles.isEmpty) return null;
    if (activeProfileId == null) {
      for (final p in profiles) {
        if (p.isSelf) return p;
      }
      return profiles.first;
    }
    for (final p in profiles) {
      if (p.id == activeProfileId) return p;
    }
    return null;
  }

  FamilyProfilesState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    List<PatientProfileDto>? profiles,
    Object? activeProfileId = _unset,
    bool? creating,
    Object? createError = _unset,
  }) =>
      FamilyProfilesState(
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        offline: offline ?? this.offline,
        error: identical(error, _unset) ? this.error : error as String?,
        profiles: profiles ?? this.profiles,
        activeProfileId: identical(activeProfileId, _unset)
            ? this.activeProfileId
            : activeProfileId as String?,
        creating: creating ?? this.creating,
        createError: identical(createError, _unset)
            ? this.createError
            : createError as String?,
      );
}

/// Owns the signed-in account's own profile plus every dependent it
/// guardians (`GET /patients/me/profiles`), and which one is currently
/// "active" for Home/booking purposes — the Flutter half of
/// `docs/CHECKPOINT.md`'s §5b multi-profile/family-support feature. Lives
/// for the whole signed-in session (constructed once in `_SignedInRootState`,
/// alongside `ProfileCubit`/`HomeCubit`), so the active-profile choice
/// survives switching tabs and resets naturally on logout when the whole
/// signed-in subtree is torn down.
class FamilyProfilesCubit extends Cubit<FamilyProfilesState> {
  FamilyProfilesCubit(this.repo, {this.connectivity})
      : super(const FamilyProfilesState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        load();
      }
    });
  }
  final FamilyProfilesRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;

  Future<void> load() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final cached = await repo.cachedList();
    if (isClosed) return;
    if (cached.isNotEmpty) {
      emit(state.copyWith(
          loading: false,
          refreshing: true,
          offline: false,
          error: null,
          profiles: cached));
    } else {
      emit(state.copyWith(
          loading: true, refreshing: false, offline: false, error: null));
    }
    try {
      final profiles = await repo.list();
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        profiles: profiles,
      ));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: cached.isNotEmpty && _isOfflineError(e),
        error: _message(e, 'Unable to load your family profiles.'),
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  /// `null` switches back to the caller's own self profile. Any other value
  /// must already be in `state.profiles` — the switcher UI only ever offers
  /// ids drawn from that list, so this is a defensive check, not a real
  /// authorization boundary (the backend's own RLS-scoped lookup is that).
  void setActiveProfile(String? id) {
    if (id == state.activeProfileId) return;
    if (id != null && !state.profiles.any((p) => p.id == id)) return;
    emit(state.copyWith(activeProfileId: id));
  }

  /// Returns the created dependent on success, or `null` on failure (the
  /// caller reads `state.createError` for the message). Guards against a
  /// double-submit itself — `FamilyProfilesRepository.createDependent` is
  /// non-idempotent-by-nature (a dependent profile can never be deleted once
  /// created), so a disabled-while-creating button is not enough on its own.
  Future<PatientProfileDto?> createDependent({
    required String fullName,
    required String dateOfBirth,
    required String gender,
    required String relationship,
    String? bloodGroup,
    String? weightKg,
    String? emergencyContactPhone,
    String? emergencyContactRelation,
  }) async {
    if (state.creating) return null;
    emit(state.copyWith(creating: true, createError: null));
    try {
      final created = await repo.createDependent(
        fullName: fullName,
        dateOfBirth: dateOfBirth,
        gender: gender,
        relationship: relationship,
        bloodGroup: bloodGroup,
        weightKg: weightKg,
        emergencyContactPhone: emergencyContactPhone,
        emergencyContactRelation: emergencyContactRelation,
      );
      if (isClosed) return created;
      emit(state.copyWith(
        creating: false,
        profiles: [...state.profiles, created],
      ));
      return created;
    } catch (e) {
      if (isClosed) return null;
      emit(state.copyWith(
          creating: false,
          createError: _message(e, 'Unable to add this dependent.')));
      return null;
    }
  }

  bool _isOfflineError(Object e) => e is ApiException && e.status == null;
  String _message(Object e, String fallback) =>
      e is ApiException ? e.userMessage : fallback;

  @override
  Future<void> close() {
    _connectivitySubscription?.cancel();
    return super.close();
  }
}
