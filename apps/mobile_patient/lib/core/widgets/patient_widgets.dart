import 'dart:async';
import 'package:flutter/material.dart';
import '../../l10n/app_localizations.dart';
import '../theme/tokens.dart';

class PatientCard extends StatelessWidget {
  const PatientCard(
      {super.key,
      required this.child,
      this.padding = const EdgeInsets.all(16)});
  final Widget child;
  final EdgeInsets padding;
  @override
  Widget build(BuildContext c) => Card(
      shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
          side: const BorderSide(color: PatientTokens.border)),
      child: Padding(padding: padding, child: child));
}

class StatusChip extends StatelessWidget {
  const StatusChip(this.text,
      {super.key,
      this.color = PatientTokens.primaryLight,
      this.textColor = PatientTokens.primaryDark});
  final String text;
  final Color color, textColor;
  @override
  Widget build(BuildContext c) => Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 5),
      decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(PatientTokens.chipRadius)),
      child: Text(text,
          style: TextStyle(
              fontSize: 11, fontWeight: FontWeight.w600, color: textColor)));
}

/// Header shared by the Patient screens — same non-AppBar approach as
/// `apps/mobile`'s `DoctorPageHeader`, ported from the prototype's shared
/// header styling rather than a platform AppBar, for visual consistency
/// with the Doctor app and the prototype itself.
class PatientPageHeader extends StatelessWidget {
  const PatientPageHeader(this.title, this.subtitle,
      {super.key, this.onRefresh, this.actions = const []});
  final String title, subtitle;
  final VoidCallback? onRefresh;
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Container(
      width: double.infinity,
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 10),
      decoration: const BoxDecoration(
          color: PatientTokens.surface,
          border: Border(bottom: BorderSide(color: PatientTokens.border))),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style: const TextStyle(
                  fontSize: 20,
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.text,
                  letterSpacing: -0.3)),
          const SizedBox(height: 2),
          Text(subtitle,
              style:
                  const TextStyle(fontSize: 12, color: PatientTokens.textSub))
        ])),
        ...actions,
        if (onRefresh != null)
          IconButton(
              onPressed: onRefresh,
              icon: const Icon(Icons.refresh, size: 18),
              color: PatientTokens.primaryDark)
      ]));
}

/// Shared notification-bell entry point: an icon with an unread-count badge.
/// Deliberately a dumb, stateless widget — the unread count and tap
/// behavior are supplied by whoever places it (see
/// `features/notifications/notification_cubit.dart`), so this file doesn't
/// need to depend on that feature.
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
          backgroundColor: PatientTokens.danger,
          child: const Icon(Icons.notifications_outlined, size: 20)),
      color: PatientTokens.primaryDark);
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
            const Icon(Icons.cloud_off, color: PatientTokens.textSub, size: 34),
            const SizedBox(height: 8),
            Text(message, textAlign: TextAlign.center),
            if (onRetry != null)
              TextButton(
                  onPressed: onRetry,
                  child: Text(AppLocalizations.of(c).retryButton))
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
            color: PatientTokens.danger.withValues(alpha: .08),
            borderRadius: BorderRadius.circular(10)),
        child: Row(children: [
          const Icon(Icons.error_outline,
              color: PatientTokens.danger, size: 18),
          const SizedBox(width: 8),
          Expanded(child: Text(message)),
          if (onRetry != null)
            TextButton(
                onPressed: onRetry,
                child: Text(AppLocalizations.of(context).retryButton)),
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
          color: PatientTokens.text.withValues(alpha: .06),
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 8),
          child: Row(children: [
            const Icon(Icons.cloud_off, size: 16, color: PatientTokens.textSub),
            const SizedBox(width: 8),
            Expanded(
                child: Text(time == null
                    ? AppLocalizations.of(context).cacheOffline
                    : AppLocalizations.of(context).cacheOfflineUpdated(time))),
          ]),
        )
      else if (time != null)
        Align(
            alignment: Alignment.centerLeft,
            child: Padding(
                padding: const EdgeInsets.fromLTRB(18, 6, 18, 2),
                child: Text(AppLocalizations.of(context).cacheLastUpdated(time),
                    style: const TextStyle(
                        fontSize: 11, color: PatientTokens.textSub)))),
    ]);
  }
}

/// Shimmer-free skeleton loading placeholder (prototype view-14) — a plain
/// pulsing `AnimatedOpacity` block, deliberately not the `shimmer` package,
/// per the plan's dependency-footprint decision.
class SkeletonBlock extends StatefulWidget {
  const SkeletonBlock(
      {super.key, this.width = double.infinity, this.height = 14});
  final double width;
  final double height;
  @override
  State<SkeletonBlock> createState() => _SkeletonBlockState();
}

class _SkeletonBlockState extends State<SkeletonBlock> {
  bool _dim = false;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _tick();
  }

  void _tick() {
    _timer = Timer(const Duration(milliseconds: 700), () {
      if (!mounted) return;
      setState(() => _dim = !_dim);
      _tick();
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedOpacity(
        opacity: _dim ? 0.4 : 0.9,
        duration: const Duration(milliseconds: 700),
        child: Container(
          width: widget.width,
          height: widget.height,
          decoration: BoxDecoration(
            color: PatientTokens.surfaceSubtle,
            borderRadius: BorderRadius.circular(6),
          ),
        ),
      );
}

/// A blocking "this action needs a connection" sheet — mirrors the
/// prototype's `#offlineActionGuardModal` for an action a patient tried to
/// take while offline (e.g. booking, uploading intake photos).
Future<void> showOfflineActionGuard(BuildContext context, String action) {
  return showModalBottomSheet(
    context: context,
    builder: (context) => Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(Icons.wifi_off_rounded,
              color: PatientTokens.warning, size: 32),
          const SizedBox(height: 12),
          Text(AppLocalizations.of(context).offlineGuardTitle(action),
              style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.w800,
                  color: PatientTokens.text)),
          const SizedBox(height: 6),
          Text(AppLocalizations.of(context).offlineGuardMessage,
              style: const TextStyle(color: PatientTokens.textSub)),
          const SizedBox(height: 16),
          SizedBox(
            width: double.infinity,
            child: FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              style: FilledButton.styleFrom(
                  backgroundColor: PatientTokens.primary),
              child: Text(AppLocalizations.of(context).okButton),
            ),
          ),
        ],
      ),
    ),
  );
}

class PatientField extends StatelessWidget {
  const PatientField(
      {super.key,
      required this.label,
      required this.controller,
      this.hint,
      this.keyboardType,
      this.obscureText = false,
      this.readOnly = false,
      this.onTap,
      this.suffixIcon,
      this.errorText,
      this.onChanged});
  final String label;
  final TextEditingController controller;
  final String? hint, errorText;
  final TextInputType? keyboardType;
  final bool obscureText, readOnly;
  final VoidCallback? onTap;
  final Widget? suffixIcon;
  final ValueChanged<String>? onChanged;
  @override
  Widget build(BuildContext context) =>
      Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text(label,
            style: const TextStyle(
                fontSize: 13,
                fontWeight: FontWeight.w600,
                color: PatientTokens.text)),
        const SizedBox(height: 7),
        TextField(
            controller: controller,
            keyboardType: keyboardType,
            obscureText: obscureText,
            readOnly: readOnly,
            onTap: onTap,
            onChanged: onChanged,
            decoration: InputDecoration(
                hintText: hint, suffixIcon: suffixIcon, errorText: errorText))
      ]);
}

class PatientSectionTitle extends StatelessWidget {
  const PatientSectionTitle(this.title,
      {super.key, this.action, this.onAction});
  final String title;
  final String? action;
  final VoidCallback? onAction;
  @override
  Widget build(BuildContext context) => Row(children: [
        Expanded(
            child: Text(title, style: Theme.of(context).textTheme.titleMedium)),
        if (action != null)
          TextButton(onPressed: onAction, child: Text(action!))
      ]);
}

class PatientFilterButton extends StatelessWidget {
  const PatientFilterButton(
      {super.key,
      required this.label,
      required this.onTap,
      this.icon = Icons.tune_rounded});
  final String label;
  final VoidCallback onTap;
  final IconData icon;
  @override
  Widget build(BuildContext context) => OutlinedButton.icon(
      onPressed: onTap,
      icon: Icon(icon, size: 17),
      label: Text(label, maxLines: 1, softWrap: false),
      style: OutlinedButton.styleFrom(
          minimumSize: const Size(0, 48),
          padding: const EdgeInsets.symmetric(horizontal: 12)));
}

class EmptyState extends StatelessWidget {
  const EmptyState(
      {super.key,
      required this.icon,
      required this.title,
      required this.message});
  final IconData icon;
  final String title, message;
  @override
  Widget build(BuildContext context) => Center(
      child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            Icon(icon, color: PatientTokens.textSub, size: 30),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 5),
            Text(message,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodyMedium)
          ])));
}
