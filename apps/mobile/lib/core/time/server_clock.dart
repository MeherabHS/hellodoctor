import 'dart:io' show HttpDate;

import 'package:flutter/foundation.dart';

/// The server's clock (product owner, 2026-11-24: server time, Kenya,
/// decides). Learned from the `Date` header of every live API response, so a
/// phone set to the wrong time still shows the right deadlines. Until the
/// first response it is the device clock.
class ServerClock {
  ServerClock._();
  static Duration _offset = Duration.zero;

  /// The server's current time (UTC).
  static DateTime now() => DateTime.now().toUtc().add(_offset);

  /// Records the server time from a response's `Date` header.
  static void observe(String? dateHeader, {DateTime? receivedAt}) {
    if (dateHeader == null || dateHeader.isEmpty) return;
    try {
      final server = HttpDate.parse(dateHeader).toUtc();
      final offset = server.difference((receivedAt ?? DateTime.now()).toUtc());
      // The header has one-second resolution: ignore sub-second noise.
      _offset = offset.inSeconds.abs() < 2 ? Duration.zero : offset;
    } catch (_) {
      // A malformed header changes nothing.
    }
  }

  @visibleForTesting
  static void set(Duration offset) => _offset = offset;
}
