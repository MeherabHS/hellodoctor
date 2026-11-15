import '../../core/notifications/notification_taps.dart';
import 'notification_navigation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/l10n/kenya_time.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/afya_widgets.dart';
import '../../core/widgets/patient_widgets.dart';
import '../../l10n/app_localizations.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';

const _monthNames = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];

/// Shown in Kenya time (EAT) like every other appointment time in the app,
/// whatever the device's own time zone is.
String _mediumDateTime(BuildContext context, DateTime utc) {
  final t = kenyaTime(utc);
  if (Localizations.localeOf(context).languageCode == 'sw') {
    final material = MaterialLocalizations.of(context);
    return '${material.formatMediumDate(t)} \u00b7 ${material.formatTimeOfDay(TimeOfDay.fromDateTime(t))}';
  }
  final hour = t.hour % 12 == 0 ? 12 : t.hour % 12;
  final minute = t.minute.toString().padLeft(2, '0');
  final suffix = t.hour >= 12 ? 'PM' : 'AM';
  return '${_monthNames[t.month - 1]} ${t.day} · $hour:$minute $suffix';
}

/// Owner-scoped inbox. Explicit taps use the same authorization checks as OS
/// notification taps; receipts alone never navigate or mark notices read.
class NotificationCenterPage extends StatefulWidget {
  const NotificationCenterPage({super.key, required this.cubit, this.deps});
  final NotificationCubit cubit;

  /// Session dependencies for appointment destinations (waiting room,
  /// outcome). Without them an appointment opens the plain status page.
  final AppointmentRouteDeps? deps;

  @override
  State<NotificationCenterPage> createState() => _NotificationCenterPageState();
}

class _NotificationCenterPageState extends State<NotificationCenterPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => widget.cubit.load());
  }

  final _tapGate = NotificationTapGate();
  Future<void> _handleTap(NotificationDto n) async {
    try {
      await openNotificationDestination(context, widget.cubit, n.id, _tapGate,
          fromInbox: true, deps: widget.deps);
    } catch (_) {}
  }

  @override
  void dispose() {
    _tapGate.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return BlocProvider.value(
      value: widget.cubit,
      child: BlocBuilder<NotificationCubit, NotificationState>(
        builder: (context, s) {
          final l10n = AppLocalizations.of(context);
          final items = [...s.items]
            ..sort((a, b) => b.createdAt.compareTo(a.createdAt));
          return Scaffold(
            backgroundColor: PatientTokens.bg,
            body: SafeArea(
              bottom: false,
              child: Column(children: [
                _NotificationsHeader(
                  title: l10n.notificationsTitle,
                  subtitle: l10n.notificationsSubtitle,
                  onRefresh: widget.cubit.refresh,
                  action: s.unreadCount > 0
                      ? TextButton(
                          onPressed: s.busy ? null : widget.cubit.markAllRead,
                          child: s.busy
                              ? const SizedBox(
                                  width: 16,
                                  height: 16,
                                  child:
                                      CircularProgressIndicator(strokeWidth: 2))
                              : Text(l10n.notificationsMarkAllRead,
                                  style: const TextStyle(fontSize: 12)),
                        )
                      : null,
                ),
                CacheStatusBar(
                    offline: false,
                    refreshing: s.refreshing,
                    lastUpdated: s.lastUpdatedAt),
                Expanded(
                  child: s.loading && items.isEmpty
                      ? const Center(child: CircularProgressIndicator())
                      : s.error != null && items.isEmpty
                          ? ErrorPanel(l10n.notificationsLoadError,
                              onRetry: widget.cubit.load)
                          : RefreshIndicator(
                              onRefresh: widget.cubit.refresh,
                              child: items.isEmpty && s.error == null
                                  ? _EmptyNotifications(
                                      title: l10n.notificationsEmpty,
                                      body: l10n.notificationsEmptyBody)
                                  : ListView(
                                      padding: const EdgeInsets.fromLTRB(
                                          0, 4, 0, 24),
                                      children: [
                                        if (s.error != null)
                                          InlineError(
                                              l10n.notificationsRefreshError,
                                              onRetry: widget.cubit.refresh),
                                        if (s.hasOlder)
                                          TextButton(
                                              onPressed: s.loadingOlder
                                                  ? null
                                                  : widget.cubit.loadOlder,
                                              child: Text(s.loadingOlder
                                                  ? l10n
                                                      .notificationsLoadingOlder
                                                  : l10n
                                                      .notificationsLoadOlder)),
                                        ...items.map((n) => _NotificationTile(
                                            key: ValueKey(n.id),
                                            notification: n,
                                            onTap: () => _handleTap(n))),
                                      ],
                                    ),
                            ),
                ),
              ]),
            ),
          );
        },
      ),
    );
  }
}

class _NotificationTile extends StatelessWidget {
  const _NotificationTile(
      {super.key, required this.notification, required this.onTap});
  final NotificationDto notification;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final n = notification;
    final unread = !n.isRead;
    return InkWell(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.fromLTRB(18, 0, 18, 10),
        padding: const EdgeInsets.all(14),
        decoration: BoxDecoration(
          color: unread
              ? PatientTokens.primaryLight.withValues(alpha: .35)
              : PatientTokens.surface,
          border: Border.all(color: PatientTokens.border),
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
        ),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Padding(
            padding: const EdgeInsets.only(top: 6, right: 8),
            child: unread
                ? Container(
                    width: 8,
                    height: 8,
                    decoration: const BoxDecoration(
                        color: PatientTokens.primary, shape: BoxShape.circle))
                : const SizedBox(width: 8, height: 8),
          ),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(n.title,
                  style: TextStyle(
                      fontSize: 13,
                      fontWeight: unread ? FontWeight.w800 : FontWeight.w600,
                      color: PatientTokens.text)),
              const SizedBox(height: 4),
              Text(n.body,
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.textSub, height: 1.4)),
              const SizedBox(height: 6),
              Text(_mediumDateTime(context, n.createdAt),
                  style: const TextStyle(
                      fontSize: 10, color: PatientTokens.textSub)),
            ]),
          ),
        ]),
      ),
    );
  }
}

/// Header of the product owner's notifications design: the brand mark over a
/// soft mint wash, then a back arrow, the title block and a refresh action.
class _NotificationsHeader extends StatelessWidget {
  const _NotificationsHeader(
      {required this.title,
      required this.subtitle,
      required this.onRefresh,
      this.action});
  final String title, subtitle;
  final VoidCallback onRefresh;
  final Widget? action;
  @override
  Widget build(BuildContext context) => Container(
        decoration: const BoxDecoration(
            gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [Color(0xFFE4F5EC), Colors.white],
                stops: [0, .55]),
            border: Border(bottom: BorderSide(color: PatientTokens.border))),
        padding: const EdgeInsets.fromLTRB(4, 8, 8, 12),
        child: Column(children: [
          const AfyaBrandLockup(markSize: 32, color: PatientTokens.primaryDark),
          const SizedBox(height: 10),
          Row(children: [
            if (Navigator.of(context).canPop())
              IconButton(
                  onPressed: () => Navigator.maybePop(context),
                  tooltip: MaterialLocalizations.of(context).backButtonTooltip,
                  icon: const Icon(Icons.arrow_back_rounded,
                      color: PatientTokens.primaryDark))
            else
              const SizedBox(width: 16),
            const SizedBox(width: 4),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: const TextStyle(
                            fontSize: 24,
                            height: 1.15,
                            fontWeight: FontWeight.w800,
                            letterSpacing: -.3,
                            color: PatientTokens.text)),
                    const SizedBox(height: 2),
                    Text(subtitle,
                        style: const TextStyle(
                            fontSize: 13.5, color: PatientTokens.textSub)),
                  ]),
            ),
            if (action != null) action!,
            IconButton(
                onPressed: onRefresh,
                tooltip: MaterialLocalizations.of(context)
                    .refreshIndicatorSemanticLabel,
                icon: const Icon(Icons.refresh_rounded,
                    size: 26, color: PatientTokens.primaryDark)),
          ]),
        ]),
      );
}

/// Empty state of the notifications design: illustration, heading, one
/// sentence, and the wave artwork resting on the bottom edge. It stays
/// scrollable so pull-to-refresh works, and it scrolls on short screens.
class _EmptyNotifications extends StatelessWidget {
  const _EmptyNotifications({required this.title, required this.body});
  final String title, body;
  @override
  Widget build(BuildContext context) => LayoutBuilder(builder: (context, box) {
        return SingleChildScrollView(
          physics: const AlwaysScrollableScrollPhysics(),
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: box.maxHeight),
            child: Column(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(28, 28, 28, 20),
                    child: Column(children: [
                      Image.asset('assets/illustrations/woman_phone_bell.webp',
                          height: 210,
                          fit: BoxFit.contain,
                          excludeFromSemantics: true),
                      const SizedBox(height: 22),
                      Text(title,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                              fontSize: 25,
                              fontWeight: FontWeight.w800,
                              letterSpacing: -.3,
                              color: PatientTokens.text)),
                      const SizedBox(height: 12),
                      Text(body,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                              fontSize: 15.5,
                              height: 1.5,
                              color: PatientTokens.textSub)),
                    ]),
                  ),
                  Image.asset('assets/illustrations/wave_footer.webp',
                      width: double.infinity,
                      fit: BoxFit.fitWidth,
                      excludeFromSemantics: true),
                ]),
          ),
        );
      });
}
