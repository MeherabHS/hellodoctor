import 'package:flutter/material.dart';
import '../theme/tokens.dart';

class DoctorCard extends StatelessWidget {
  const DoctorCard(
      {super.key,
      required this.child,
      this.padding = const EdgeInsets.all(14)});
  final Widget child;
  final EdgeInsets padding;
  @override
  Widget build(BuildContext c) => Card(
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(DoctorTokens.radius),
          side: const BorderSide(color: DoctorTokens.border)),
      child: Padding(padding: padding, child: child));
}

class StatusChip extends StatelessWidget {
  const StatusChip(this.text,
      {super.key,
      this.color = DoctorTokens.primaryLight,
      this.textColor = DoctorTokens.primaryDark});
  final String text;
  final Color color, textColor;
  @override
  Widget build(BuildContext c) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 5),
      decoration:
          BoxDecoration(color: color, borderRadius: BorderRadius.circular(20)),
      child: Text(text,
          style: TextStyle(
              fontSize: 11, fontWeight: FontWeight.w700, color: textColor)));
}

/// Header shared by the Doctor Mobile screens — ports prototype/index.html's
/// `.dir-header`/`.dir-title`/`.dir-sub` (line ~796) exactly, rather than a
/// platform AppBar.
class DoctorPageHeader extends StatelessWidget {
  const DoctorPageHeader(this.title, this.subtitle,
      {super.key, this.onRefresh, this.actions = const []});
  final String title, subtitle;
  final VoidCallback? onRefresh;

  /// Extra trailing icons (e.g. the notification bell) shown before the
  /// refresh button, so screens needn't each rebuild the header's layout.
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 10),
      decoration: const BoxDecoration(
          color: DoctorTokens.surface,
          border: Border(bottom: BorderSide(color: DoctorTokens.border))),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style: const TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w800,
                  color: DoctorTokens.text,
                  letterSpacing: -0.3)),
          const SizedBox(height: 2),
          Text(subtitle,
              style: const TextStyle(fontSize: 12, color: DoctorTokens.textSub))
        ])),
        ...actions,
        if (onRefresh != null)
          IconButton(
              onPressed: onRefresh,
              icon: const Icon(Icons.refresh, size: 18),
              color: DoctorTokens.primaryDark)
      ]));
}

/// Shared notification-bell entry point: an icon with an unread-count badge,
/// meant to be dropped into any `DoctorPageHeader.actions`. Deliberately a
/// dumb, stateless widget — the unread count and tap behavior are supplied
/// by whoever places it (see `features/notifications/notification_cubit.dart`),
/// so this file doesn't need to depend on that feature.
class NotificationBellAction extends StatelessWidget {
  const NotificationBellAction(
      {super.key, required this.unreadCount, required this.onTap});
  final int unreadCount;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => IconButton(
      onPressed: onTap,
      icon: Badge(
          isLabelVisible: unreadCount > 0,
          label: Text(unreadCount > 99 ? '99+' : '$unreadCount'),
          backgroundColor: DoctorTokens.danger,
          child: const Icon(Icons.notifications_outlined, size: 20)),
      color: DoctorTokens.primaryDark);
}

class ErrorPanel extends StatelessWidget {
  const ErrorPanel(this.message, {super.key, this.onRetry});
  final String message;
  final VoidCallback? onRetry;
  @override
  Widget build(BuildContext c) => Center(
      child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            const Icon(Icons.cloud_off, color: DoctorTokens.textSub, size: 34),
            const SizedBox(height: 8),
            Text(message, textAlign: TextAlign.center),
            if (onRetry != null)
              TextButton(onPressed: onRetry, child: const Text('Retry'))
          ])));
}

class InlineError extends StatelessWidget {
  const InlineError(this.message, {super.key, this.onRetry});
  final String message;
  final VoidCallback? onRetry;
  @override
  Widget build(BuildContext context) => Container(
        margin: const EdgeInsets.fromLTRB(18, 8, 18, 4),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
            color: DoctorTokens.danger.withValues(alpha: .08),
            borderRadius: BorderRadius.circular(10)),
        child: Row(children: [
          const Icon(Icons.error_outline, color: DoctorTokens.danger, size: 18),
          const SizedBox(width: 8),
          Expanded(child: Text(message)),
          if (onRetry != null)
            TextButton(onPressed: onRetry, child: const Text('Retry')),
        ]),
      );
}

class CacheStatusBar extends StatelessWidget {
  const CacheStatusBar(
      {super.key,
      required this.offline,
      required this.refreshing,
      this.lastUpdated});
  final bool offline, refreshing;
  final DateTime? lastUpdated;

  @override
  Widget build(BuildContext context) {
    final updated = lastUpdated?.toLocal();
    final time = updated == null
        ? null
        : '${updated.hour.toString().padLeft(2, '0')}:${updated.minute.toString().padLeft(2, '0')}';
    return Column(children: [
      if (refreshing) const LinearProgressIndicator(minHeight: 2),
      if (offline)
        Container(
          width: double.infinity,
          color: DoctorTokens.text.withValues(alpha: .06),
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 8),
          child: Row(children: [
            const Icon(Icons.cloud_off, size: 16, color: DoctorTokens.textSub),
            const SizedBox(width: 8),
            Expanded(
                child: Text(time == null
                    ? 'Offline — showing saved data'
                    : 'Offline — showing saved data · Last updated $time')),
          ]),
        )
      else if (time != null)
        Align(
            alignment: Alignment.centerLeft,
            child: Padding(
                padding: const EdgeInsets.fromLTRB(18, 6, 18, 2),
                child: Text('Last updated $time',
                    style: const TextStyle(
                        fontSize: 11, color: DoctorTokens.textSub)))),
    ]);
  }
}
