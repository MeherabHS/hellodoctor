import 'package:flutter/material.dart';

import '../theme/tokens.dart';

/// The AfyaMkononi heart-and-figures mark from the reference image, redrawn
/// as clean vector shapes and rendered to a 512 px transparent PNG (the
/// reference's own mark is only about 65 px wide). Shape proportions are
/// matched by eye to the reference; it is a recreation, not source artwork.
class AfyaLogoMark extends StatelessWidget {
  const AfyaLogoMark({super.key, this.size = 52});
  final double size;
  @override
  Widget build(BuildContext context) => Image.asset(
        'assets/brand/afya_logo.png',
        width: size,
        height: size,
        fit: BoxFit.contain,
        filterQuality: FilterQuality.medium,
        excludeFromSemantics: true,
      );
}

/// Logo mark and "AfyaMkononi" wordmark on one line. It scales down to fit
/// whatever width it is given, so it never overflows a narrow header.
class AfyaBrandLockup extends StatelessWidget {
  const AfyaBrandLockup(
      {super.key, this.markSize = 32, this.color = PatientTokens.brandTeal});
  final double markSize;
  final Color color;
  @override
  Widget build(BuildContext context) => FittedBox(
        fit: BoxFit.scaleDown,
        child: Row(mainAxisSize: MainAxisSize.min, children: [
          AfyaLogoMark(size: markSize),
          const SizedBox(width: 8),
          Text('AfyaMkononi',
              maxLines: 1,
              style: TextStyle(
                  fontSize: markSize * .56,
                  fontWeight: FontWeight.w800,
                  color: color)),
        ]),
      );
}

/// Dark-green circle with the user's initials (reference panel 1, top right).
class AfyaInitialsAvatar extends StatelessWidget {
  const AfyaInitialsAvatar({super.key, required this.name, this.size = 36});
  final String? name;
  final double size;

  static String initialsOf(String? name) {
    final words = (name ?? '')
        .trim()
        .split(RegExp(r'\s+'))
        .where((w) => w.isNotEmpty)
        .toList();
    if (words.isEmpty) return '';
    final first = words.first.characters.first;
    final second = words.length > 1
        ? words.last.characters.first
        : (words.first.characters.length > 1
            ? words.first.characters.elementAt(1)
            : '');
    return (first + second).toUpperCase();
  }

  @override
  Widget build(BuildContext context) {
    final initials = initialsOf(name);
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: const BoxDecoration(
          color: PatientTokens.avatarGreen, shape: BoxShape.circle),
      child: initials.isEmpty
          ? Icon(Icons.person, color: Colors.white, size: size * .55)
          : Text(initials,
              textScaler: TextScaler.noScaling,
              style: TextStyle(
                  color: Colors.white,
                  fontSize: size * .36,
                  fontWeight: FontWeight.w700,
                  letterSpacing: .2)),
    );
  }
}

/// Tinted circular icon tile used by service cards and list rows.
class AfyaIconTile extends StatelessWidget {
  const AfyaIconTile(
      {super.key,
      required this.icon,
      required this.color,
      required this.background,
      this.size = 50,
      this.iconSize});
  final IconData icon;
  final Color color, background;
  final double size;
  final double? iconSize;
  @override
  Widget build(BuildContext context) => Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: BoxDecoration(color: background, shape: BoxShape.circle),
        child: Icon(icon, color: color, size: iconSize ?? size * .62),
      );
}

/// One card of the Home 3 x 3 service grid (reference panel 1).
class AfyaServiceTile extends StatelessWidget {
  const AfyaServiceTile(
      {super.key,
      required this.icon,
      required this.color,
      required this.background,
      required this.label,
      required this.onTap,
      this.enabled = true,
      this.badge});
  final IconData icon;
  final Color color, background;
  final String label;
  final VoidCallback onTap;

  /// A disabled tile is drawn in greys (offline design); it still receives
  /// the tap so the caller can explain why.
  final bool enabled;

  /// Small pill under the label, e.g. "Available offline".
  final String? badge;
  @override
  Widget build(BuildContext context) => Container(
        decoration: BoxDecoration(
            color: badge != null
                ? const Color(0xFFEAF8F0)
                : enabled
                    ? PatientTokens.surface
                    : const Color(0xFFF4F6F7),
            borderRadius: BorderRadius.circular(14),
            boxShadow: enabled ? PatientTokens.softShadow : null),
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(14),
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(14),
            child: Padding(
              padding: const EdgeInsets.fromLTRB(4, 14, 4, 12),
              child: Column(mainAxisSize: MainAxisSize.min, children: [
                AfyaIconTile(
                    icon: icon,
                    color: enabled ? color : const Color(0xFFA9B1B6),
                    background: enabled ? background : const Color(0xFFE6E9EB),
                    size: 52),
                const SizedBox(height: 9),
                Text(label,
                    textAlign: TextAlign.center,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                        fontSize: 13,
                        height: 1.25,
                        fontWeight: FontWeight.w600,
                        color: enabled
                            ? PatientTokens.ink
                            : const Color(0xFF98A2A8))),
                if (badge != null) ...[
                  const SizedBox(height: 6),
                  Container(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
                    decoration: BoxDecoration(
                        color: const Color(0xFFCDEEDB),
                        borderRadius: BorderRadius.circular(12)),
                    child: FittedBox(
                      fit: BoxFit.scaleDown,
                      child: Row(mainAxisSize: MainAxisSize.min, children: [
                        const Icon(Icons.download_done_rounded,
                            size: 13, color: PatientTokens.primaryDark),
                        const SizedBox(width: 3),
                        Text(badge!,
                            maxLines: 1,
                            style: const TextStyle(
                                fontSize: 10.5,
                                fontWeight: FontWeight.w700,
                                color: PatientTokens.primaryDark)),
                      ]),
                    ),
                  ),
                ],
              ]),
            ),
          ),
        ),
      );
}

/// Back-arrow title bar used by pushed screens (reference panels 2-8, 11).
class AfyaBackHeader extends StatelessWidget {
  const AfyaBackHeader(this.title, {super.key, this.actions = const []});
  final String title;
  final List<Widget> actions;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(6, 6, 12, 6),
        child: Row(children: [
          IconButton(
              onPressed: () => Navigator.maybePop(context),
              tooltip: MaterialLocalizations.of(context).backButtonTooltip,
              icon: const Icon(Icons.arrow_back_rounded,
                  size: 22, color: PatientTokens.text)),
          const SizedBox(width: 2),
          Expanded(
              child: Text(title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 17,
                      fontWeight: FontWeight.w700,
                      color: PatientTokens.text))),
          ...actions,
        ]),
      );
}
