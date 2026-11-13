import 'dart:async';
import 'dart:math';
import 'dart:typed_data';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../../core/network/api_client.dart';
import 'chat_models.dart';
import 'chat_repository.dart';

const _unset = Object();

class ChatState {
  const ChatState({
    this.loading = false,
    this.error,
    this.messages = const [],
    this.sending = false,
    this.notSent = false,
    this.serverLocked = false,
    this.hasOlder = true,
    this.loadingOlder = false,
    this.uploadingImage = false,
    this.imageError,
    this.imagePending = false,
  });
  final bool loading, sending, notSent, serverLocked, hasOlder, loadingOlder;
  final String? error;

  /// An image is being sent; [imageError] is why the last one was not, and
  /// [imagePending] means it is kept for a retry.
  final bool uploadingImage, imagePending;
  final String? imageError;
  final List<ChatMessageDto> messages;

  ChatState copyWith({
    bool? loading,
    Object? error = _unset,
    List<ChatMessageDto>? messages,
    bool? sending,
    bool? notSent,
    bool? serverLocked,
    bool? hasOlder,
    bool? loadingOlder,
    bool? uploadingImage,
    Object? imageError = _unset,
    bool? imagePending,
  }) =>
      ChatState(
        loading: loading ?? this.loading,
        error: identical(error, _unset) ? this.error : error as String?,
        messages: messages ?? this.messages,
        sending: sending ?? this.sending,
        notSent: notSent ?? this.notSent,
        serverLocked: serverLocked ?? this.serverLocked,
        hasOlder: hasOlder ?? this.hasOlder,
        loadingOlder: loadingOlder ?? this.loadingOlder,
        uploadingImage: uploadingImage ?? this.uploadingImage,
        imageError: identical(imageError, _unset)
            ? this.imageError
            : imageError as String?,
        imagePending: imagePending ?? this.imagePending,
      );
}

/// Owns one consultation-scoped thread. Both Patient and Doctor clients
/// refresh the same authoritative history while their threads are open.
class ChatCubit extends Cubit<ChatState> {
  ChatCubit(this.repo, {required this.conversationId})
      : super(const ChatState());
  final ChatRepository repo;
  final String conversationId;
  void Function(bool succeeded)? onRefreshOutcome;
  bool _loadInFlight = false;

  List<ChatMessageDto> merge(
      List<ChatMessageDto> previous, List<ChatMessageDto> next) {
    final all = {
      for (final m in previous) m.id: m,
      for (final m in next) m.id: m
    }.values.toList();
    all.sort((a, b) {
      final time = a.sentAt.compareTo(b.sentAt);
      return time == 0 ? a.id.compareTo(b.id) : time;
    });
    return all;
  }

  Future<void> loadOlder() async {
    if (state.loadingOlder || !state.hasOlder || state.messages.isEmpty) return;
    emit(state.copyWith(loadingOlder: true));
    try {
      final older =
          await repo.olderMessages(conversationId, state.messages.first);
      if (!isClosed) {
        emit(state.copyWith(
            messages: merge(state.messages, older),
            hasOlder: older.length == 50,
            loadingOlder: false));
      }
    } catch (e) {
      if (!isClosed) {
        emit(state.copyWith(
            loadingOlder: false,
            error: e is ApiException
                ? e.userMessage
                : 'Unable to load older messages.'));
      }
    }
  }

  Future<List<ChatMessageDto>> refreshMessages() async {
    final source = repo;
    if (state.messages.isEmpty) return source.messages(conversationId);
    final result = <ChatMessageDto>[];
    var after = state.messages.last;
    // Catch up incrementally; older pages stay in memory and are never redownloaded by a tick.
    for (var pageNumber = 0; pageNumber < 4; pageNumber++) {
      final page = await source.newerMessages(conversationId, after);
      result.addAll(page);
      if (page.length < 50) break;
      after = page.last;
    }
    return result;
  }

  Future<void> load() async {
    if (_loadInFlight) return;
    _loadInFlight = true;
    if (state.messages.isEmpty) {
      emit(state.copyWith(loading: true, error: null));
    }
    try {
      final messages = await refreshMessages();
      onRefreshOutcome?.call(true);
      if (isClosed) return;
      emit(state.copyWith(
          loading: false,
          error: null,
          messages: merge(state.messages, messages),
          hasOlder:
              state.messages.isEmpty ? messages.length == 50 : state.hasOlder));
    } catch (e) {
      onRefreshOutcome?.call(false);
      if (isClosed) return;
      emit(state.copyWith(
        loading: false,
        error: e is ApiException
            ? e.userMessage
            : 'Unable to load this conversation.',
      ));
    } finally {
      _loadInFlight = false;
    }
  }

  /// Best-effort background refresh — a transient failure is silent, same
  /// rationale as `WaitingRoomCubit.poll`.
  Future<void> poll() async {
    if (_loadInFlight || isClosed) return;
    _loadInFlight = true;
    try {
      final messages = await refreshMessages();
      onRefreshOutcome?.call(true);
      if (isClosed) return;
      emit(state.copyWith(
          messages: merge(state.messages, messages),
          hasOlder:
              state.messages.isEmpty ? messages.length == 50 : state.hasOlder));
    } catch (_) {
      onRefreshOutcome?.call(false);
      // Intentionally silent — see doc comment above.
    } finally {
      _loadInFlight = false;
    }
  }

  Future<void> send(String content) async {
    final trimmed = content.trim();
    if (trimmed.isEmpty || state.sending || state.serverLocked) return;
    emit(state.copyWith(sending: true));
    try {
      await repo.sendMessage(conversationId, trimmed);
      if (isClosed) return;
      emit(state.copyWith(sending: false, notSent: false));
      await load();
    } catch (e) {
      if (isClosed) return;
      emit(state.copyWith(
          sending: false,
          notSent: true,
          serverLocked:
              e is ApiException && (e.status == 403 || e.status == 409)));
    }
  }

  ({Uint8List bytes, String mime, String key})? _pendingImage;

  /// Sends a JPEG or PNG. On failure the image and its request key are kept,
  /// so [retryImage] cannot post it twice.
  Future<void> sendImage(Uint8List bytes, String mimeType) async {
    if (state.uploadingImage || state.serverLocked) return;
    final key = List.generate(
            16,
            (_) =>
                Random.secure().nextInt(256).toRadixString(16).padLeft(2, '0'))
        .join();
    _pendingImage = (bytes: bytes, mime: mimeType, key: key);
    await retryImage();
  }

  Future<void> retryImage() async {
    final pending = _pendingImage;
    if (pending == null || state.uploadingImage || isClosed) return;
    emit(state.copyWith(uploadingImage: true, imageError: null));
    try {
      final message = await repo.uploadClinicalImage(
          conversationId, pending.bytes, pending.mime, pending.key);
      _pendingImage = null;
      if (isClosed) return;
      emit(state.copyWith(
          uploadingImage: false,
          imagePending: false,
          messages: merge(state.messages, [message])));
    } catch (e) {
      if (isClosed) return;
      final locked = e is ApiException && e.status == 403;
      if (locked) _pendingImage = null;
      emit(state.copyWith(
          uploadingImage: false,
          imagePending: !locked,
          serverLocked: locked || state.serverLocked,
          imageError: e is ApiException ? e.userMessage : ''));
    }
  }
}
