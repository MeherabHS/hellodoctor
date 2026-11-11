import 'dart:async';

bool notificationUuid(String? value) =>
    value != null &&
    RegExp(r'^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$')
        .hasMatch(value);

/// Only an opaque inbox ID crosses the OS boundary. URLs, entity IDs and
/// notification copy from a push are never navigation authority.
String? notificationTapId(Object? payload) {
  final value = payload is Map ? payload['notification_id'] : payload;
  if (value is! String) return null;
  final id =
      value.startsWith('notification_event:') ? value.substring(19) : value;
  return notificationUuid(id) ? id : null;
}

/// In-memory handoff while the authenticated shell is starting. Contains no
/// message text or clinical data and is cleared at the account boundary.
class NotificationTaps {
  static final _events = StreamController<String?>.broadcast(sync: true);
  static final _pending = <String?>[];
  static Stream<String?> get events => _events.stream;
  static void receive(Object? payload) {
    final id = notificationTapId(payload);
    if (_events.hasListener) {
      _events.add(id);
    } else {
      if (_pending.length == 8) _pending.removeAt(0);
      _pending.add(id);
    }
  }

  static List<String?> drain() {
    final result = List<String?>.from(_pending);
    _pending.clear();
    return result;
  }

  static void clear() => _pending.clear();
}

/// Coalesces duplicate native callbacks and fences asynchronous work when an
/// authenticated shell is disposed. Failed opens remain retryable.
class NotificationTapGate {
  NotificationTapGate({DateTime Function()? now}) : _now = now ?? DateTime.now;
  final DateTime Function() _now;
  final _active = <String>{};
  final _opened = <String, DateTime>{};
  bool disposed = false;
  bool get current => !disposed;
  Future<void> run(String? id, Future<void> Function() open) async {
    final key = id ?? 'inbox';
    if (disposed ||
        _active.contains(key) ||
        (_opened[key] != null &&
            _now().difference(_opened[key]!) < const Duration(seconds: 2))) {
      return;
    }
    _active.add(key);
    try {
      await open();
      if (!disposed) {
        if (_opened.length == 128) _opened.remove(_opened.keys.first);
        _opened[key] = _now();
      }
    } finally {
      _active.remove(key);
    }
  }

  void dispose() {
    disposed = true;
    _active.clear();
    _opened.clear();
  }
}
