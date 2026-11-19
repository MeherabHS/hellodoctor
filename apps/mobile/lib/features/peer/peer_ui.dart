import 'package:flutter/material.dart';

import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/person_photo.dart';

/// Shared look of the Peer Navigator screens, measured from the Care-app
/// peer references in `docs/design/peer-navigator/care-app/`.
class PeerTokens {
  static const green = Color(0xFF087A45); // buttons, active tab, numbers
  static const mint = Color(0xFFE6F6EB); // hero and banner cards
  static const mintSoft = Color(0xFFF1FAF5); // highlighted rows
  static const mintLine = Color(0xFFBFE3CD);
  static const navy = Color(0xFF0B1026);
  static const grey = Color(0xFF5B6272);
  static const line = Color(0xFFE3E7EC);
  static const field = Color(0xFFF4F6F8);
  static const amber = Color(0xFFFFF4DC);
  static const amberText = Color(0xFF9A6200);
  static const blue = Color(0xFFE6F1FD);
  static const blueText = Color(0xFF1D5FB8);
  static const purple = Color(0xFFF0E7FD);
  static const purpleText = Color(0xFF7B3FD1);
  static const peach = Color(0xFFFFEBD9);
  static const red = Color(0xFFD92D20);
  static const redSoft = Color(0xFFFEF2F2);
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

String _two(int v) => v.toString().padLeft(2, '0');
String peerTime(DateTime t) {
  final h = t.hour % 12 == 0 ? 12 : t.hour % 12;
  return '$h:${_two(t.minute)} ${t.hour < 12 ? 'AM' : 'PM'}';
}

const peerMonths = [
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
];
const peerWeekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

DateTime peerDay(DateTime t) => DateTime(t.year, t.month, t.day);
bool peerSameDay(DateTime a, DateTime b) =>
    a.year == b.year && a.month == b.month && a.day == b.day;

/// "Today", "Tomorrow", "Yesterday" or "Mon 16 Nov".
String peerDayLabel(DateTime t, {DateTime? now}) {
  final today = peerDay(now ?? DateTime.now());
  final diff = peerDay(t).difference(today).inDays;
  if (diff == 0) return 'Today';
  if (diff == 1) return 'Tomorrow';
  if (diff == -1) return 'Yesterday';
  return '${peerWeekdays[t.weekday - 1]} ${t.day} ${peerMonths[t.month - 1]}';
}

/// "Sun, 15 Nov 2026".
String peerLongDate(DateTime t) =>
    '${peerWeekdays[t.weekday - 1]}, ${t.day} ${peerMonths[t.month - 1]} ${t.year}';

/// Chat-list time: clock time today, "Yesterday", weekday this week, else date.
String peerListTime(DateTime t, {DateTime? now}) {
  final today = peerDay(now ?? DateTime.now());
  final diff = today.difference(peerDay(t)).inDays;
  if (diff <= 0) return peerTime(t);
  if (diff == 1) return 'Yesterday';
  if (diff < 7) return peerWeekdays[t.weekday - 1];
  return '${t.day} ${peerMonths[t.month - 1]}';
}

/// "5 min ago", "3 h ago", else "Yesterday, 3:30 PM" / "12 Nov, 3:30 PM".
String peerAgo(DateTime t, {DateTime? now}) {
  final n = now ?? DateTime.now();
  final d = n.difference(t);
  if (d.inMinutes < 1) return 'Just now';
  if (d.inMinutes < 60) return '${d.inMinutes} min ago';
  if (peerSameDay(t, n)) return '${d.inHours} h ago';
  final label = peerDayLabel(t, now: n);
  return '${label == 'Yesterday' ? label : '${t.day} ${peerMonths[t.month - 1]}'}, ${peerTime(t)}';
}

String peerInitials(String name) {
  final parts = name.trim().split(RegExp(r'\s+')).where((p) => p.isNotEmpty);
  return parts.take(2).map((p) => p[0].toUpperCase()).join();
}

String peerErrorText(Object e) =>
    e is ApiException ? e.userMessage : 'Something went wrong. Please retry.';

void peerToast(BuildContext context, String text) =>
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));

// ---------------------------------------------------------------------------
// Widgets
// ---------------------------------------------------------------------------

/// Initials on a soft colour picked from the name (the reference rotates
/// mint, purple, blue and peach).
///
/// With [api] and [photoPath], shows the person's saved photo instead once
/// it loads (see [PersonPhoto]).
class PeerAvatar extends StatelessWidget {
  const PeerAvatar(this.name,
      {super.key, this.size = 52, this.api, this.photoPath});
  final String name;
  final double size;
  final ApiClient? api;
  final String? photoPath;
  static const _palette = [
    (PeerTokens.mint, PeerTokens.green),
    (PeerTokens.purple, PeerTokens.purpleText),
    (PeerTokens.blue, Color(0xFF0B1026)),
    (PeerTokens.peach, Color(0xFF0B1026)),
  ];
  @override
  Widget build(BuildContext context) {
    final (bg, fg) = _palette[
        name.codeUnits.fold<int>(0, (a, b) => a + b) % _palette.length];
    final initials = Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: BoxDecoration(color: bg, shape: BoxShape.circle),
        child: Text(peerInitials(name),
            style: TextStyle(
                color: fg,
                fontWeight: FontWeight.w800,
                fontSize: size * 0.36)));
    final client = api;
    if (client == null || photoPath == null) return initials;
    return PersonPhoto(
        api: client, path: photoPath, size: size, fallback: initials);
  }
}

/// Screen title, subtitle and round icon buttons (search, filter, calendar).
class PeerHeader extends StatelessWidget {
  const PeerHeader(
      {super.key,
      required this.title,
      required this.subtitle,
      this.actions = const [],
      this.onBack});
  final String title, subtitle;
  final List<Widget> actions;
  final VoidCallback? onBack;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(18, 14, 18, 12),
      child: Row(crossAxisAlignment: CrossAxisAlignment.center, children: [
        if (onBack != null) ...[
          PeerCircleButton(
              icon: Icons.arrow_back_rounded,
              tooltip: 'Back',
              onTap: onBack!,
              filled: true),
          const SizedBox(width: 12),
        ],
        Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(title,
              style: const TextStyle(
                  fontSize: 23,
                  height: 1.15,
                  fontWeight: FontWeight.w800,
                  color: PeerTokens.navy)),
          const SizedBox(height: 3),
          Text(subtitle,
              style: const TextStyle(fontSize: 12.5, color: PeerTokens.grey)),
        ])),
        for (final a in actions) ...[const SizedBox(width: 8), a],
      ]));
}

class PeerCircleButton extends StatelessWidget {
  const PeerCircleButton(
      {super.key,
      required this.icon,
      required this.tooltip,
      required this.onTap,
      this.filled = false,
      this.active = false});
  final IconData icon;
  final String tooltip;
  final VoidCallback onTap;
  final bool filled, active;
  @override
  Widget build(BuildContext context) => Tooltip(
      message: tooltip,
      child: Material(
          color: active
              ? PeerTokens.mint
              : filled
                  ? PeerTokens.mintSoft
                  : Colors.white,
          shape: CircleBorder(
              side: BorderSide(
                  color: filled ? Colors.transparent : PeerTokens.line)),
          child: InkWell(
              customBorder: const CircleBorder(),
              onTap: onTap,
              child: SizedBox(
                  width: 46,
                  height: 46,
                  child: Icon(icon,
                      size: 22,
                      color: filled ? PeerTokens.green : PeerTokens.navy)))));
}

/// Pill tabs: the selected one is solid green with a white count badge.
class PeerPillTabs extends StatelessWidget {
  const PeerPillTabs(
      {super.key,
      required this.labels,
      required this.selected,
      required this.onSelected,
      this.counts = const {},
      this.icons = const {}});
  final List<String> labels;
  final int selected;
  final ValueChanged<int> onSelected;
  final Map<int, int> counts;
  final Map<int, IconData> icons;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 18),
      child: Row(children: [
        for (var i = 0; i < labels.length; i++) ...[
          if (i > 0) const SizedBox(width: 8),
          Expanded(
              child: Material(
                  color: i == selected ? PeerTokens.green : Colors.white,
                  shape: StadiumBorder(
                      side: BorderSide(
                          color: i == selected
                              ? PeerTokens.green
                              : PeerTokens.line)),
                  child: InkWell(
                      key: ValueKey('peer-pill-${labels[i]}'),
                      customBorder: const StadiumBorder(),
                      onTap: () => onSelected(i),
                      child: SizedBox(
                          height: 40,
                          child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                if (icons[i] != null) ...[
                                  Icon(icons[i],
                                      size: 18,
                                      color: i == selected
                                          ? Colors.white
                                          : PeerTokens.navy),
                                  const SizedBox(width: 6),
                                ],
                                Flexible(
                                    child: Text(labels[i],
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                            fontSize: 13.5,
                                            fontWeight: i == selected
                                                ? FontWeight.w800
                                                : FontWeight.w600,
                                            color: i == selected
                                                ? Colors.white
                                                : PeerTokens.navy))),
                                if (counts[i] != null) ...[
                                  const SizedBox(width: 6),
                                  Container(
                                      constraints:
                                          const BoxConstraints(minWidth: 22),
                                      height: 22,
                                      alignment: Alignment.center,
                                      padding: const EdgeInsets.symmetric(
                                          horizontal: 6),
                                      decoration: BoxDecoration(
                                          color: i == selected
                                              ? Colors.white
                                              : PeerTokens.field,
                                          borderRadius:
                                              BorderRadius.circular(11)),
                                      child: Text('${counts[i]}',
                                          style: TextStyle(
                                              fontSize: 12,
                                              fontWeight: FontWeight.w800,
                                              color: i == selected
                                                  ? PeerTokens.green
                                                  : PeerTokens.navy))),
                                ],
                              ]))))),
        ],
      ]));
}

/// Two-option segmented control (Create Slots / My Schedule).
class PeerSegment extends StatelessWidget {
  const PeerSegment(
      {super.key,
      required this.items,
      required this.selected,
      required this.onSelected});
  final List<(IconData, String)> items;
  final int selected;
  final ValueChanged<int> onSelected;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 18),
      child: Container(
          height: 50,
          decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(25),
              border: Border.all(color: PeerTokens.line)),
          child: Row(children: [
            for (var i = 0; i < items.length; i++)
              Expanded(
                  child: InkWell(
                      key: ValueKey('peer-segment-${items[i].$2}'),
                      borderRadius: BorderRadius.circular(25),
                      onTap: () => onSelected(i),
                      child: Container(
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                              color: i == selected
                                  ? PeerTokens.green
                                  : Colors.transparent,
                              borderRadius: BorderRadius.circular(25)),
                          child: Row(
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Icon(items[i].$1,
                                    size: 20,
                                    color: i == selected
                                        ? Colors.white
                                        : PeerTokens.navy),
                                const SizedBox(width: 8),
                                Flexible(
                                    child: Text(items[i].$2,
                                        maxLines: 1,
                                        overflow: TextOverflow.ellipsis,
                                        style: TextStyle(
                                            fontSize: 15,
                                            fontWeight: FontWeight.w700,
                                            color: i == selected
                                                ? Colors.white
                                                : PeerTokens.navy))),
                              ])))),
          ])));
}

/// Status pill with an icon (Pending clock, Accepted check, …).
class PeerStatusPill extends StatelessWidget {
  const PeerStatusPill(this.status,
      {super.key, this.pendingLabel = 'Pending', this.label});
  final String status;
  final String pendingLabel;

  /// Replaces the status word (e.g. "Approved" on the profile).
  final String? label;
  @override
  Widget build(BuildContext context) {
    final (label, icon, bg, fg) = switch (status) {
      'PENDING' => (
          pendingLabel,
          Icons.schedule_rounded,
          PeerTokens.amber,
          PeerTokens.amberText
        ),
      'ACCEPTED' => (
          'Accepted',
          Icons.check_rounded,
          PeerTokens.mint,
          PeerTokens.green
        ),
      'COMPLETED' => (
          'Completed',
          Icons.task_alt_rounded,
          PeerTokens.blue,
          PeerTokens.blueText
        ),
      'DECLINED' => (
          'Declined',
          Icons.close_rounded,
          PeerTokens.redSoft,
          PeerTokens.red
        ),
      'CANCELLED' => (
          'Cancelled',
          Icons.block_rounded,
          PeerTokens.field,
          PeerTokens.grey
        ),
      _ => (status, Icons.info_outline, PeerTokens.field, PeerTokens.grey),
    };
    return Container(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration:
            BoxDecoration(color: bg, borderRadius: BorderRadius.circular(20)),
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          Icon(icon, size: 15, color: fg),
          const SizedBox(width: 5),
          Text(this.label ?? label,
              style: TextStyle(
                  fontSize: 12, fontWeight: FontWeight.w700, color: fg)),
        ]));
  }
}

/// White card with a hairline border and soft shadow; optional mint wave in
/// the bottom-right corner as in the reference cards.
class PeerCard extends StatelessWidget {
  const PeerCard(
      {super.key,
      required this.child,
      this.onTap,
      this.color = Colors.white,
      this.padding = const EdgeInsets.all(16),
      this.wave = false,
      this.border,
      this.radius = 22});
  final Widget child;
  final VoidCallback? onTap;
  final Color color;
  final EdgeInsets padding;
  final bool wave;
  final Color? border;
  final double radius;
  @override
  Widget build(BuildContext context) => Container(
      decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(radius),
          boxShadow: color == Colors.white
              ? const [
                  BoxShadow(
                      color: Color(0x0F0B1026),
                      blurRadius: 14,
                      offset: Offset(0, 4))
                ]
              : null),
      child: Material(
          color: color,
          shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(radius),
              side: BorderSide(
                  color: border ??
                      (color == Colors.white
                          ? PeerTokens.line
                          : Colors.transparent))),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
              onTap: onTap,
              child: Stack(fit: StackFit.passthrough, children: [
                if (wave)
                  const Positioned.fill(
                      child: IgnorePointer(
                          child: CustomPaint(painter: PeerWavePainter()))),
                Padding(padding: padding, child: child),
              ]))));
}

/// The soft mint swoosh in the bottom-right corner of reference cards.
class PeerWavePainter extends CustomPainter {
  const PeerWavePainter({this.color = const Color(0xFFD9F1E2)});
  final Color color;
  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width, h = size.height;
    final path = Path()
      ..moveTo(w * 0.62, h)
      ..quadraticBezierTo(w * 0.84, h * 0.9, w, h * 0.82)
      ..lineTo(w, h)
      ..close();
    canvas.drawPath(path, Paint()..color = color.withValues(alpha: 0.75));
  }

  @override
  bool shouldRepaint(covariant PeerWavePainter old) => old.color != color;
}

/// Illustration, title, body and an outlined call to action.
class PeerEmptyState extends StatelessWidget {
  const PeerEmptyState(
      {super.key,
      required this.asset,
      required this.title,
      required this.body,
      this.action,
      this.actionIcon,
      this.onAction,
      this.footer});
  final String asset, title, body;
  final String? action;
  final IconData? actionIcon;
  final VoidCallback? onAction;
  final Widget? footer;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.fromLTRB(28, 24, 28, 24),
      child: Column(children: [
        Image.asset(asset, height: 220, fit: BoxFit.contain),
        const SizedBox(height: 18),
        Text(title,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 23,
                fontWeight: FontWeight.w800,
                color: PeerTokens.navy)),
        const SizedBox(height: 10),
        Text(body,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 15, height: 1.45, color: PeerTokens.grey)),
        if (action != null) ...[
          const SizedBox(height: 22),
          OutlinedButton.icon(
              style: OutlinedButton.styleFrom(
                  foregroundColor: PeerTokens.green,
                  minimumSize: const Size(220, 52),
                  side: const BorderSide(color: PeerTokens.green, width: 1.4),
                  shape: const StadiumBorder(),
                  textStyle: const TextStyle(
                      fontSize: 16, fontWeight: FontWeight.w800)),
              onPressed: onAction,
              icon: Icon(actionIcon ?? Icons.chevron_right_rounded),
              label: Text(action!)),
        ],
        if (footer != null) ...[const SizedBox(height: 16), footer!],
      ]));
}

/// Large filled green action button.
class PeerPrimaryButton extends StatelessWidget {
  const PeerPrimaryButton(
      {super.key,
      required this.label,
      required this.onPressed,
      this.icon,
      this.busy = false,
      this.height = 54});
  final String label;
  final VoidCallback? onPressed;
  final IconData? icon;
  final bool busy;
  final double height;
  @override
  Widget build(BuildContext context) => SizedBox(
      height: height,
      child: FilledButton(
          style: FilledButton.styleFrom(
              backgroundColor: PeerTokens.green,
              disabledBackgroundColor: PeerTokens.green.withValues(alpha: 0.4),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(16)),
              textStyle: TextStyle(
                  fontSize: height <= 46 ? 14 : 16,
                  fontWeight: FontWeight.w800)),
          onPressed: busy ? null : onPressed,
          child: busy
              ? const SizedBox(
                  width: 22,
                  height: 22,
                  child: CircularProgressIndicator(
                      strokeWidth: 2.4, color: Colors.white))
              : Row(mainAxisSize: MainAxisSize.min, children: [
                  if (icon != null) ...[Icon(icon), const SizedBox(width: 10)],
                  Flexible(child: Text(label, overflow: TextOverflow.ellipsis)),
                ])));
}

/// White button with a red label (Decline).
class PeerDangerOutlineButton extends StatelessWidget {
  const PeerDangerOutlineButton(
      {super.key,
      required this.label,
      required this.onPressed,
      this.height = 54});
  final String label;
  final VoidCallback? onPressed;
  final double height;
  @override
  Widget build(BuildContext context) => SizedBox(
      height: height,
      child: OutlinedButton.icon(
          style: OutlinedButton.styleFrom(
              foregroundColor: PeerTokens.red,
              backgroundColor: Colors.white,
              side: const BorderSide(color: Color(0xFFCDD2D9)),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(16)),
              textStyle: TextStyle(
                  fontSize: height <= 46 ? 14 : 16,
                  fontWeight: FontWeight.w700)),
          onPressed: onPressed,
          icon: Icon(Icons.close_rounded, size: height <= 46 ? 20 : 24),
          label: Text(label)));
}

/// Shared rest-of-screen loader with retry.
class PeerLoader<T> extends StatelessWidget {
  const PeerLoader(
      {super.key,
      required this.future,
      required this.builder,
      required this.onRetry});
  final Future<T> future;
  final Widget Function(T data) builder;
  final VoidCallback onRetry;
  @override
  Widget build(BuildContext context) => FutureBuilder<T>(
      future: future,
      builder: (context, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const Padding(
              padding: EdgeInsets.all(40),
              child: Center(child: CircularProgressIndicator()));
        }
        if (snap.hasError) {
          return Padding(
              padding: const EdgeInsets.all(28),
              child: Column(children: [
                Text(peerErrorText(snap.error!),
                    textAlign: TextAlign.center,
                    style: const TextStyle(color: DoctorTokens.slate)),
                TextButton(onPressed: onRetry, child: const Text('Retry')),
              ]));
        }
        return builder(snap.data as T);
      });
}

/// A one-field dialog. Returns the text on confirm, null on cancel. The
/// dialog owns its controller, so it is disposed only after the closing
/// animation.
Future<String?> peerPromptText(BuildContext context,
        {required String title,
        String initial = '',
        String? hint,
        int maxLength = 300,
        int lines = 1,
        String confirm = 'Save',
        Key? fieldKey,
        Key? confirmKey}) =>
    showDialog<String>(
        context: context,
        builder: (_) => _PromptDialog(
            title: title,
            initial: initial,
            hint: hint,
            maxLength: maxLength,
            lines: lines,
            confirm: confirm,
            fieldKey: fieldKey,
            confirmKey: confirmKey));

class _PromptDialog extends StatefulWidget {
  const _PromptDialog(
      {required this.title,
      required this.initial,
      required this.hint,
      required this.maxLength,
      required this.lines,
      required this.confirm,
      this.fieldKey,
      this.confirmKey});
  final String title, initial, confirm;
  final String? hint;
  final int maxLength, lines;
  final Key? fieldKey, confirmKey;
  @override
  State<_PromptDialog> createState() => _PromptDialogState();
}

class _PromptDialogState extends State<_PromptDialog> {
  late final controller = TextEditingController(text: widget.initial);
  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
          title: Text(widget.title),
          content: TextField(
              key: widget.fieldKey,
              controller: controller,
              autofocus: true,
              maxLength: widget.maxLength,
              minLines: widget.lines,
              maxLines: widget.lines,
              decoration: InputDecoration(hintText: widget.hint)),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(context),
                child: const Text('Cancel')),
            TextButton(
                key: widget.confirmKey,
                onPressed: () => Navigator.pop(context, controller.text),
                child: Text(widget.confirm)),
          ]);
}
