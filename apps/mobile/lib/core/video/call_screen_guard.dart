import 'dart:async';

import 'package:flutter/material.dart';

/// Guards a "join/rejoin a live video consultation" entry point against a
/// rapid double-tap pushing two routes for the same call. Each push
/// independently constructs its own `CallController`/Agora engine, requests
/// camera/mic permission, and mints its own RTC token — a second,
/// near-simultaneous tap (easy on a laggy device, before the first route's
/// transition visually starts) would otherwise run two camera/engine
/// instances concurrently and could publish a duplicate/garbled feed into
/// the same channel.
///
/// Share one instance across every "join"/"rejoin" entry point that can lead
/// to the same call screen (a consultation card's own button, the
/// queue-level "Join"/"Start" action, a chat thread's "Start Video Call"
/// icon, and the consultation detail page's own "Join Agora" button all
/// ultimately reach the same `AgoraPage`/`ConsultationPage`). [busy] is set
/// the instant any guarded push starts and cleared only once that pushed
/// route is popped, so joining a second call is also correctly blocked for
/// as long as a call screen from a prior join is still on screen, not just
/// during the transition itself.
class CallScreenGuard {
  bool _busy = false;
  bool get busy => _busy;

  void push(BuildContext context, WidgetBuilder builder) {
    if (_busy) return;
    _busy = true;
    unawaited(Navigator.push(context, MaterialPageRoute(builder: builder))
        .whenComplete(() => _busy = false));
  }
}
