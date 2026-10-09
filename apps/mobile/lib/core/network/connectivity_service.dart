import 'dart:async';
import 'package:connectivity_plus/connectivity_plus.dart';

enum NetworkStatus { unknown, online, offline }

/// Connectivity is a UX hint only. HTTP responses remain authoritative.
class ConnectivityService {
  ConnectivityService([Connectivity? connectivity])
      : _connectivity = connectivity ?? Connectivity() {
    _subscription = _connectivity.onConnectivityChanged.listen((result) {
      _status = _fromResult(result);
      _changes.add(_status);
    });
    _prime();
  }

  final Connectivity _connectivity;
  final _changes = StreamController<NetworkStatus>.broadcast();
  late final StreamSubscription<List<ConnectivityResult>> _subscription;
  NetworkStatus _status = NetworkStatus.unknown;

  NetworkStatus get status => _status;
  bool get isOffline => _status == NetworkStatus.offline;
  Stream<NetworkStatus> get changes => _changes.stream;

  Future<void> _prime() async {
    try {
      _status = _fromResult(await _connectivity.checkConnectivity());
      if (!_changes.isClosed) _changes.add(_status);
    } catch (_) {
      // Unknown keeps the app willing to make the first authoritative request.
    }
  }

  static NetworkStatus _fromResult(List<ConnectivityResult> results) {
    if (results.isEmpty || results.every((r) => r == ConnectivityResult.none)) {
      return NetworkStatus.offline;
    }
    return NetworkStatus.online;
  }

  Future<void> dispose() async {
    await _subscription.cancel();
    await _changes.close();
  }
}
