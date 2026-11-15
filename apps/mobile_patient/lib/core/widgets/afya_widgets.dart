import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../theme/tokens.dart';
import '../utils/initials.dart';

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
    final words = initialWords(name);
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
      this.badge,
      this.labelWidth});
  final IconData icon;

  /// The label's width, from a parent grid that knows it (rows here use
  /// IntrinsicHeight). Without it the label measures itself.
  final double? labelWidth;
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
                // Up to three lines, never splitting a word: longer Swahili
                // labels and large system text shrink to fit.
                AfyaFitText(label,
                    maxLines: 3,
                    maxWidth: labelWidth,
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

/// A short label for a fixed-width tile. With large system text or long
/// Swahili words it shrinks just enough that no word is split across lines
/// and nothing is cut off, down to [minScale] of its size; past that it
/// ellipsizes.
class AfyaFitText extends StatelessWidget {
  const AfyaFitText(this.text,
      {super.key,
      required this.style,
      this.maxLines = 2,
      this.textAlign = TextAlign.center,
      this.minScale = 0.7,
      this.maxWidth});
  final String text;

  /// The width the label gets. Pass it where a parent measures intrinsic
  /// sizes (e.g. IntrinsicHeight), which cannot host a LayoutBuilder.
  final double? maxWidth;
  final TextStyle style;
  final int maxLines;
  final TextAlign textAlign;
  final double minScale;

  @override
  Widget build(BuildContext context) => maxWidth != null
      ? _fit(context, maxWidth!)
      : LayoutBuilder(builder: (context, c) => _fit(context, c.maxWidth));

  Widget _fit(BuildContext context, double available) {
    final scaler = MediaQuery.textScalerOf(context);
    final direction = Directionality.of(context);
    final base = DefaultTextStyle.of(context).style.merge(style);
    final size = base.fontSize ?? 14;
    final width = available - 0.5;
    TextPainter paint(String t, double f, {int? lines}) => TextPainter(
        text: TextSpan(text: t, style: base.copyWith(fontSize: size * f)),
        textDirection: direction,
        textScaler: scaler,
        maxLines: lines)
      ..layout(maxWidth: lines == null ? double.infinity : width);
    var factor = 1.0;
    if (width.isFinite && width > 0) {
      for (final word in text.split(RegExp(r'\s+'))) {
        if (word.isEmpty) continue;
        final w = paint(word, 1).width;
        if (w > width) factor = math.min(factor, width / w);
      }
      while (factor > minScale &&
          paint(text, factor, lines: maxLines).didExceedMaxLines) {
        factor -= 0.04;
      }
      factor = math.max(factor, minScale);
    }
    return Text(text,
        textAlign: textAlign,
        maxLines: maxLines,
        overflow: TextOverflow.ellipsis,
        style: style.copyWith(fontSize: size * factor));
  }
}
