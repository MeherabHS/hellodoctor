import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'api_client.dart';

/// One signal from the live chat socket: "conversation X has something new",
/// or a resync after a gap (refresh whatever is open). Never message text.
class ChatNudge {
  const ChatNudge(this.conversationId, {this.kind = ''});
  const ChatNudge.resync()
      : conversationId = null,
        kind = '';
  final String? conversationId;
  final String kind;
  bool get isResync => conversationId == null;

  /// Whether a screen showing [conversation] should refresh.
  bool concerns(String conversation) =>
      isResync || conversationId == conversation;
}

typedef LiveSocketConnector = Future<WebSocket> Function(
    Uri uri, Map<String, String> headers);

/// Live chat nudges (product owner, 2026-11-21). While signed in, in the
/// foreground and with a chat screen open (conversation or chat list), the
/// app keeps one WebSocket to `/chat/live`; a nudge makes the open chat
/// screen fetch over the normal HTTP endpoint at once. With no chat screen
/// open there is no socket (closed-app delivery is push's job). Chat screens
/// keep polling as the fallback (slowly while [connected], at their normal
/// pace otherwise), so a blocked or dropped socket never loses messages.
class ChatLive with WidgetsBindingObserver {
  ChatLive(this._api,
      {LiveSocketConnector? connect, this.observeLifecycle = true})
      : _connectSocket = connect ??
            ((uri, headers) =>
                WebSocket.connect(uri.toString(), headers: headers)
                    .timeout(const Duration(seconds: 15)));

  /// The signed-in session's instance; null while signed out.
  static ChatLive? current;

  /// Poll interval chat screens use while the socket is up.
  static const fallbackPoll = Duration(seconds: 30);

  final ApiClient _api;
  final LiveSocketConnector _connectSocket;
  final bool observeLifecycle;
  final _nudges = StreamController<ChatNudge>.broadcast();
  final connected = ValueNotifier<bool>(false);
  final _random = math.Random();

  WebSocket? _socket;
  StreamSubscription<dynamic>? _frames;
  Timer? _retry;
  bool _running = false;
  bool _connecting = false;
  bool _foreground = true;
  int _failures = 0;
  int _holders = 0;

  bool get _wanted => _running && _foreground && _holders > 0;

  Stream<ChatNudge> get nudges => _nudges.stream;

  void start() {
    if (_running) return;
    _running = true;
    if (observeLifecycle) WidgetsBinding.instance.addObserver(this);
    unawaited(_connect());
  }

  /// A chat screen opened: connect if this is the first.
  void _acquire() {
    _holders++;
    unawaited(_connect());
  }

  /// A chat screen closed: disconnect when it was the last.
  void _release() {
    if (_holders > 0) _holders--;
    if (_holders == 0) {
      _retry?.cancel();
      unawaited(_drop());
    }
  }

  Future<void> stop() async {
    _running = false;
    if (observeLifecycle) WidgetsBinding.instance.removeObserver(this);
    _retry?.cancel();
    await _drop();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _foreground = state == AppLifecycleState.resumed;
    if (_foreground) {
      _failures = 0;
      unawaited(_connect());
    } else {
      _retry?.cancel();
      unawaited(_drop());
    }
  }

  Future<void> _drop() async {
    final socket = _socket;
    _socket = null;
    await _frames?.cancel();
    _frames = null;
    connected.value = false;
    try {
      await socket?.close();
    } catch (_) {}
  }

  Future<WebSocket?> _open() async {
    final live = await _api.liveSocket();
    if (live == null) return null;
    return _connectSocket(live.uri, live.headers);
  }

  Future<void> _connect() async {
    if (!_wanted || _socket != null || _connecting) return;
    _connecting = true;
    try {
      WebSocket? socket;
      try {
        socket = await _open();
      } catch (_) {
        // A refused upgrade is usually an expired access token: refresh it
        // once through the normal session flow, then try again.
        if (_failures > 0) rethrow;
        await _api.refreshSession();
        socket = await _open();
      }
      if (socket == null || !_wanted) {
        await socket?.close();
        return;
      }
      socket.pingInterval = const Duration(seconds: 30);
      _socket = socket;
      _failures = 0;
      connected.value = true;
      // Anything sent while disconnected is fetched now.
      _nudges.add(const ChatNudge.resync());
      _frames = socket.listen(_onFrame,
          onDone: _lost, onError: (Object _) => _lost(), cancelOnError: true);
    } catch (_) {
      _failures++;
      _schedule();
    } finally {
      _connecting = false;
    }
  }

  void _onFrame(dynamic data) {
    if (data is! String || data.length > 2048) return;
    try {
      final frame = jsonDecode(data);
      if (frame is! Map) return;
      switch (frame['type']) {
        case 'chat':
          final id = frame['conversation_id'];
          if (id is String && id.isNotEmpty) {
            _nudges.add(ChatNudge(id, kind: '${frame['kind'] ?? ''}'));
          }
        case 'resync':
          _nudges.add(const ChatNudge.resync());
      }
    } catch (_) {/* ignore a malformed frame */}
  }

  void _lost() {
    _socket = null;
    _frames = null;
    connected.value = false;
    _schedule();
  }

  /// Reconnect soon after a normal close (the server closes every few
  /// minutes), with a doubling, jittered backoff after failures.
  void _schedule() {
    if (!_wanted) return;
    _retry?.cancel();
    final seconds = _failures == 0 ? 1 : math.min(60, 1 << _failures);
    final jitter = _random.nextInt(1000);
    _retry = Timer(Duration(milliseconds: seconds * 1000 + jitter),
        () => unawaited(_connect()));
  }

  @visibleForTesting
  int get failures => _failures;

  /// For tests: behave as if a chat screen were open.
  @visibleForTesting
  void holdForTest() => _acquire();
}

/// What a chat screen holds: runs [onNudge] for nudges it [wants], and
/// [onConnectionChange] when the socket connects or drops (so polling can
/// slow down or catch up). Safe when signed out (does nothing).
class LiveChatHook {
  LiveChatHook({
    required bool Function(ChatNudge nudge) wants,
    required void Function() onNudge,
    void Function()? onConnectionChange,
  })  : _live = ChatLive.current,
        _onConnectionChange = onConnectionChange {
    _live?._acquire();
    _subscription = _live?.nudges.listen((nudge) {
      if (wants(nudge)) onNudge();
    });
    if (onConnectionChange != null) {
      _live?.connected.addListener(onConnectionChange);
    }
  }

  final ChatLive? _live;
  final void Function()? _onConnectionChange;
  StreamSubscription<ChatNudge>? _subscription;

  /// True while nudges are arriving, so polling can be slow.
  bool get live => _live?.connected.value ?? false;

  /// [normal] while the socket is down, the slow fallback while it is up.
  Duration pollEvery(Duration normal) =>
      live && normal < ChatLive.fallbackPoll ? ChatLive.fallbackPoll : normal;

  void dispose() {
    _live?._release();
    unawaited(_subscription?.cancel());
    final listener = _onConnectionChange;
    if (listener != null) _live?.connected.removeListener(listener);
  }
}
