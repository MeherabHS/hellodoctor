import 'dart:async';
import 'dart:typed_data';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import 'profile_models.dart';
import 'profile_repository.dart';

const _unset = Object();

class ProfileState {
  const ProfileState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.profile,
    this.saving = false,
    this.lastUpdatedAt,
    this.photoLoading = false,
    this.photoSaving = false,
    this.photoBytes,
    this.photoError,
  });
  final bool loading, refreshing, offline, saving;
  final String? error;
  final PatientProfileDto? profile;
  final DateTime? lastUpdatedAt;

  /// Photo state is deliberately independent of [error]/[saving] (the rest
  /// of the profile form) — a failed photo upload must never be mistaken
  /// for a failed name/DOB/etc. save, and must never block either one.
  final bool photoLoading, photoSaving;
  final Uint8List? photoBytes;
  final String? photoError;

  ProfileState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    Object? profile = _unset,
    bool? saving,
    DateTime? lastUpdatedAt,
    bool? photoLoading,
    bool? photoSaving,
    Object? photoBytes = _unset,
    Object? photoError = _unset,
  }) =>
      ProfileState(
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        offline: offline ?? this.offline,
        error: identical(error, _unset) ? this.error : error as String?,
        profile: identical(profile, _unset)
            ? this.profile
            : profile as PatientProfileDto?,
        saving: saving ?? this.saving,
        lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
        photoLoading: photoLoading ?? this.photoLoading,
        photoSaving: photoSaving ?? this.photoSaving,
        photoBytes: identical(photoBytes, _unset)
            ? this.photoBytes
            : photoBytes as Uint8List?,
        photoError: identical(photoError, _unset)
            ? this.photoError
            : photoError as String?,
      );
}

class ProfileCubit extends Cubit<ProfileState> {
  ProfileCubit(this.repo, {this.connectivity}) : super(const ProfileState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        load();
      }
    });
  }
  final ProfileRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;

  Future<void> load() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final cached = await repo.cachedProfile();
    if (isClosed) return;
    if (cached != null) {
      emit(state.copyWith(
          loading: false,
          refreshing: true,
          offline: false,
          error: null,
          profile: cached));
    } else {
      emit(state.copyWith(
          loading: true, refreshing: false, offline: false, error: null));
    }
    try {
      final profile = await repo.profile();
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        profile: profile,
        lastUpdatedAt: DateTime.now().toUtc(),
      ));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: cached != null && _isOfflineError(e),
        error: _message(e, 'Unable to load your profile.'),
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  /// Returns true on success. The caller (profile page / completion page)
  /// decides what to do next (e.g. pop, show a confirmation) — this cubit
  /// only owns the save-in-flight/error state.
  Future<bool> save({
    String? fullName,
    String? dateOfBirth,
    String? gender,
    String? bloodGroup,
    String? weightKg,
    String? emergencyContactPhone,
    String? emergencyContactRelation,
  }) async {
    if (state.saving) return false;
    emit(state.copyWith(saving: true, error: null));
    try {
      await repo.updateProfile(
        fullName: fullName,
        dateOfBirth: dateOfBirth,
        gender: gender,
        bloodGroup: bloodGroup,
        weightKg: weightKg,
        emergencyContactPhone: emergencyContactPhone,
        emergencyContactRelation: emergencyContactRelation,
      );
      final profile = await repo.profile();
      if (isClosed) return true;
      emit(state.copyWith(
          saving: false,
          profile: profile,
          lastUpdatedAt: DateTime.now().toUtc()));
      return true;
    } catch (e) {
      if (isClosed) return false;
      emit(state.copyWith(
          saving: false, error: _message(e, 'Unable to save your profile.')));
      return false;
    }
  }

  /// Loads (or reloads, e.g. after an upload/removal) the caller's own
  /// photo bytes. Silently leaves [ProfileState.photoBytes] at whatever it
  /// already was on failure — a transient failure to *load* the current
  /// avatar is not worth surfacing as a blocking error on a profile screen
  /// that otherwise loaded fine; the user can simply retry viewing it.
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

  /// Returns true on success. Upload failure (malformed image, oversized
  /// after the server's own compression attempt, network, ...) is reported
  /// via [ProfileState.photoError] only — it must never affect [error] or
  /// [saving], since a profile picture is optional and its failure must
  /// never block anything else on this screen.
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

  bool _isOfflineError(Object e) => e is ApiException && e.status == null;
  String _message(Object e, String fallback) =>
      e is ApiException ? e.userMessage : fallback;

  @override
  Future<void> close() {
    _connectivitySubscription?.cancel();
    return super.close();
  }
}
