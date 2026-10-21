import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/widgets.dart';

import 'connectivity_service.dart';

final screenRouteObserver = RouteObserver<ModalRoute<dynamic>>();

/// A screen owns this coordinator. Cached data is never an authorization grant.
class ScreenRefresh with WidgetsBindingObserver, RouteAware {
  ScreenRefresh(
    this.refresh, {
    required this.interval,
    this.enabled,
    this.hasError,
    Stream<NetworkStatus>? networkChanges,
    Duration? maxInterval,
    math.Random? random,
  })  : maxInterval = maxInterval ?? interval * 8,
        _random = random ?? math.Random() {
    WidgetsBinding.instance.addObserver(this);
    _network = networkChanges?.listen((state) {
      if (state == NetworkStatus.online) _recover();
    });
    _start();
  }
  final FutureOr<void> Function() refresh;
  final Duration interval;
  final bool Function()? enabled;

  /// Optional: lets a screen/cubit that already tracks its own error state
  /// report whether the most recently completed [refresh] left it in an
  /// error state. [refresh] itself is not expected to throw (most callers
  /// already catch their own exceptions so a failed poll doesn't crash an
  /// unawaited timer tick) — this is the cooperative signal that lets
  /// ScreenRefresh tell a swallowed failure apart from a real success.
  /// A thrown exception from [refresh] also counts as a failure. A
  /// successful/clean refresh resets the backoff. Screens that omit this
  /// keep the original fixed-[interval] behavior.
  final bool Function()? hasError;
  final Duration maxInterval;
  final math.Random _random;
  int _consecutiveFailures = 0;

  ModalRoute<dynamic>? _route;
  Timer? _timer;
  StreamSubscription<NetworkStatus>? _network;
  Future<void>? _inFlight;
  bool _disposed = false;
  bool _recovering = false;
  int _timerVersion = 0;
  bool get visible =>
      !_disposed &&
      (_route == null || _route!.isCurrent) &&
      (WidgetsBinding.instance.lifecycleState == null ||
          WidgetsBinding.instance.lifecycleState ==
              AppLifecycleState.resumed) &&
      (enabled?.call() ?? true);

  /// The delay before the next scheduled poll: [interval] with no recent
  /// failures, otherwise a doubling backoff capped at [maxInterval] plus up
  /// to 20% bounded jitter, so many clients recovering from the same outage
  /// don't retry in lockstep.
  Duration get currentInterval {
    if (_consecutiveFailures == 0) return interval;
    final shift = _consecutiveFailures.clamp(0, 6);
    var backoff = interval * (1 << shift);
    if (backoff > maxInterval) backoff = maxInterval;
    final jitterBoundMs = (backoff.inMilliseconds * 0.2).round();
    final jitterMs = jitterBoundMs > 0 ? _random.nextInt(jitterBoundMs) : 0;
    return backoff + Duration(milliseconds: jitterMs);
  }

  void attach(BuildContext context) {
    final next = ModalRoute.of(context);
    if (identical(next, _route)) return;
    if (_route != null) screenRouteObserver.unsubscribe(this);
    _route = next;
    if (next != null) screenRouteObserver.subscribe(this, next);
  }

  void _start() {
    if (_timer != null || _disposed) return;
    _arm(interval);
  }

  void _arm(Duration delay, {bool recovery = false}) {
    if (_disposed) return;
    _timer?.cancel();
    _recovering = recovery;
    final version = ++_timerVersion;
    _timer = Timer(delay, () {
      if (_disposed || version != _timerVersion) return;
      _timer = null;
      _recovering = false;
      final ran = visible ? run() : Future<void>.value();
      // Explicit refresh callers still receive errors. Automatic recovery must
      // keep its bounded cadence without an unhandled asynchronous exception.
      unawaited(
        ran.catchError((Object _) {}).whenComplete(() {
          if (!_disposed && version == _timerVersion && _timer == null) {
            _arm(currentInterval);
          }
        }),
      );
    });
  }

  void _recover() {
    if (!visible || _recovering) return;
    if (_inFlight != null) {
      if (_timer == null) _arm(currentInterval);
      return;
    }
    // Shared connectivity/resume events coalesce into one short jitter timer.
    // Only a successful read may clear failure history; run() stays immediate.
    _arm(Duration(milliseconds: 250 + _random.nextInt(1751)), recovery: true);
  }

  void _stop() {
    _timerVersion++;
    _recovering = false;
    _timer?.cancel();
    _timer = null;
  }

  Future<void> run() {
    if (!visible) return Future.value();
    if (_inFlight != null) return _inFlight!;
    late final Future<void> work;
    work = Future<void>.sync(refresh).then((_) {
      _consecutiveFailures =
          (hasError?.call() ?? false) ? _consecutiveFailures + 1 : 0;
    }).catchError((Object e, StackTrace st) {
      _consecutiveFailures++;
      throw e;
    }).whenComplete(() {
      if (identical(_inFlight, work)) _inFlight = null;
    });
    _inFlight = work;
    return work;
  }

  @override
  void didPush() => _start();
  @override
  void didPushNext() => _stop();
  @override
  void didPopNext() {
    _recover();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _recover();
    } else {
      _stop();
    }
  }

  void dispose() {
    _disposed = true;
    _stop();
    _network?.cancel();
    screenRouteObserver.unsubscribe(this);
    WidgetsBinding.instance.removeObserver(this);
  }
}
