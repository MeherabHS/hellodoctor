import 'dart:async';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import 'vault_models.dart';
import 'vault_repository.dart';

const _unset = Object();

class VaultState {
  const VaultState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.documents = const [],
    this.lastUpdatedAt,
  });
  final bool loading, refreshing, offline;
  final String? error;
  final List<PatientDocumentDto> documents;
  final DateTime? lastUpdatedAt;

  VaultState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    List<PatientDocumentDto>? documents,
    DateTime? lastUpdatedAt,
  }) =>
      VaultState(
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        offline: offline ?? this.offline,
        error: identical(error, _unset) ? this.error : error as String?,
        documents: documents ?? this.documents,
        lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
      );
}

/// Owns the Health Vault list (plan's gap #2, M7). Mirrors `HomeCubit`'s
/// active-profile pattern exactly (§5b requires M7 to scope reads to the
/// currently active profile, not always "the account") — `null` = the
/// caller's own self profile.
class VaultCubit extends Cubit<VaultState> {
  VaultCubit(this.repo, {this.connectivity}) : super(const VaultState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        load();
      }
    });
  }
  final VaultRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;
  String? _activeProfileId;

  /// Re-queries the vault for a different profile (§5b). A no-op if the id
  /// hasn't actually changed.
  void setActiveProfile(String? profileId) {
    if (profileId == _activeProfileId) return;
    _activeProfileId = profileId;
    load();
  }

  Future<void> load() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final viewingSelf = _activeProfileId == null;
    final cached = viewingSelf
        ? await repo.cachedDocuments()
        : const <PatientDocumentDto>[];
    if (isClosed) return;
    if (cached.isNotEmpty) {
      emit(state.copyWith(
          loading: false,
          refreshing: true,
          offline: false,
          error: null,
          documents: cached));
    } else {
      emit(state.copyWith(
          loading: true,
          refreshing: false,
          offline: false,
          error: null,
          documents: const []));
    }
    try {
      final documents = await repo.documents(profileId: _activeProfileId);
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: false,
        error: null,
        documents: documents,
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
            : 'Unable to load your Health Vault.',
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
