import 'dart:async';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/network/api_client.dart';
import '../../../core/network/connectivity_service.dart';
import 'directory_models.dart';
import 'directory_repository.dart';

const _unset = Object();

/// `'all' | 'video' | 'chat'` — the prototype's `dirTabAll`/`dirTabChat`/
/// `dirTabVideo` modality selector (`view-3`).
class DirectoryState {
  const DirectoryState({
    this.loading = false,
    this.refreshing = false,
    this.offline = false,
    this.error,
    this.doctors = const [],
    this.modalityTab = 'all',
    this.specialty = 'all',
    this.search = '',
    this.onDutyOnly = false,
  });
  final bool loading, refreshing, offline;
  final String? error;
  final List<PublicDoctorDto> doctors;
  final String modalityTab, specialty, search;
  final bool onDutyOnly;

  /// Client-side specialty/search narrowing on top of whatever the server
  /// already returned for [modalityTab] — the specialty pills and search box
  /// both act on the currently loaded modality page, matching the
  /// prototype's instant-filter behaviour (`filterDoctorDirectory()`) rather
  /// than issuing a new request per keystroke.
  List<PublicDoctorDto> get visible {
    final q = search.trim().toLowerCase();
    return doctors.where((d) {
      final matchesSpecialty = specialty == 'all' ||
          d.primarySpecialty.toLowerCase() == specialty.toLowerCase();
      final matchesSearch = q.isEmpty ||
          d.fullName.toLowerCase().contains(q) ||
          d.primarySpecialty.toLowerCase().contains(q) ||
          d.currentHospital.toLowerCase().contains(q);
      return matchesSpecialty && matchesSearch && (!onDutyOnly || d.isOnDuty);
    }).toList();
  }

  DirectoryState copyWith({
    bool? loading,
    bool? refreshing,
    bool? offline,
    Object? error = _unset,
    List<PublicDoctorDto>? doctors,
    String? modalityTab,
    String? specialty,
    String? search,
    bool? onDutyOnly,
  }) =>
      DirectoryState(
        loading: loading ?? this.loading,
        refreshing: refreshing ?? this.refreshing,
        offline: offline ?? this.offline,
        error: identical(error, _unset) ? this.error : error as String?,
        doctors: doctors ?? this.doctors,
        modalityTab: modalityTab ?? this.modalityTab,
        specialty: specialty ?? this.specialty,
        search: search ?? this.search,
        onDutyOnly: onDutyOnly ?? this.onDutyOnly,
      );
}

class DirectoryCubit extends Cubit<DirectoryState> {
  DirectoryCubit(this.repo, {this.connectivity})
      : super(const DirectoryState()) {
    _connectivitySubscription = connectivity?.changes.listen((status) {
      if (status == NetworkStatus.offline) {
        emit(state.copyWith(offline: true));
      } else if (status == NetworkStatus.online && state.offline) {
        load();
      }
    });
  }
  final DirectoryRepository repo;
  final ConnectivityService? connectivity;
  StreamSubscription<NetworkStatus>? _connectivitySubscription;
  bool _refreshInFlight = false;

  String? _modalityQuery(String tab) => switch (tab) {
        'video' => 'VIDEO',
        'chat' => 'CHAT',
        _ => null,
      };

  Future<void> load() async {
    if (_refreshInFlight) return;
    _refreshInFlight = true;
    final cached = state.modalityTab == 'all'
        ? await repo.cachedDoctors()
        : const <PublicDoctorDto>[];
    if (isClosed) return;
    if (cached.isNotEmpty && state.doctors.isEmpty) {
      emit(state.copyWith(
          loading: false,
          refreshing: true,
          offline: false,
          error: null,
          doctors: cached));
    } else {
      emit(state.copyWith(
          loading: state.doctors.isEmpty,
          refreshing: state.doctors.isNotEmpty,
          offline: false,
          error: null));
    }
    try {
      final doctors =
          await repo.search(modality: _modalityQuery(state.modalityTab));
      if (isClosed) return;
      emit(state.copyWith(
          loading: false,
          refreshing: false,
          offline: false,
          error: null,
          doctors: doctors));
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        refreshing: false,
        offline: cached.isNotEmpty && e is ApiException && e.status == null,
        error: e is ApiException
            ? e.userMessage
            : 'Unable to load the specialist directory.',
      ));
    } finally {
      _refreshInFlight = false;
    }
  }

  void setModalityTab(String tab) {
    if (tab == state.modalityTab) return;
    emit(state.copyWith(modalityTab: tab, doctors: const []));
    load();
  }

  void setSpecialty(String specialty) =>
      emit(state.copyWith(specialty: specialty));
  void setSearch(String value) => emit(state.copyWith(search: value));
  void setOnDutyOnly(bool value) => emit(state.copyWith(onDutyOnly: value));
  void resetFilters() {
    final reload = state.modalityTab != 'all';
    emit(state.copyWith(
        specialty: 'all', search: '', onDutyOnly: false, modalityTab: 'all'));
    if (reload) load();
  }

  @override
  Future<void> close() {
    _connectivitySubscription?.cancel();
    return super.close();
  }
}
