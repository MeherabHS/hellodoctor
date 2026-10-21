import 'package:flutter/widgets.dart';
import 'connectivity_service.dart';
import 'screen_refresh.dart';

/// Fresh availability is display state; every booking is revalidated server-side.
class AvailabilityRefresh {
  AvailabilityRefresh(
      void Function() refresh, ConnectivityService? connectivity,
      {Stream<NetworkStatus>? networkChanges})
      : _refresh = ScreenRefresh(refresh,
            interval: const Duration(seconds: 30),
            enabled: () => connectivity?.isOffline != true,
            networkChanges: networkChanges ?? connectivity?.changes);
  final ScreenRefresh _refresh;
  void attach(BuildContext context) => _refresh.attach(context);
  void didChangeAppLifecycleState(AppLifecycleState state) =>
      _refresh.didChangeAppLifecycleState(state);
  void dispose() => _refresh.dispose();
}
