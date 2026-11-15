import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_bottom_nav.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../home/brand_header.dart';
import 'patient_modules.dart';

/// Frame shared by the module screens (reference panels 2-8 and 11): a
/// back-arrow title bar, the body, and the bottom navigation with Services
/// active. Tapping a tab returns to the shell and selects it.
class AfyaModuleScaffold extends StatelessWidget {
  const AfyaModuleScaffold(
      {super.key,
      required this.title,
      required this.nav,
      required this.child,
      this.selectedTab = 1,
      this.appHeader});
  final String title;
  final PatientShellNav nav;
  final Widget child;
  final int selectedTab;

  /// When set, the app's main header (logo, bell, avatar) sits above the
  /// back-arrow title, as on the HIV Prevention & Sexual Health screens.
  final AppHeaderDeps? appHeader;
  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
            bottom: false,
            child: Column(children: [
              if (appHeader != null) AfyaBrandHeader(deps: appHeader!),
              AfyaBackHeader(title.replaceAll('\n', ' ')),
              Expanded(child: child),
            ])),
        bottomNavigationBar: SafeArea(
            top: false,
            child: PatientBottomNav(
                selected: selectedTab,
                onSelected: (i) {
                  Navigator.of(context).popUntil((r) => r.isFirst);
                  nav.select(i);
                })),
      );
}

/// Pill tabs: the active one is solid green, the others are soft white pills.
class AfyaSegmentedTabs extends StatelessWidget {
  const AfyaSegmentedTabs(
      {super.key,
      required this.labels,
      required this.selected,
      required this.onSelected,
      this.expand = false});
  final List<String> labels;
  final int selected;
  final ValueChanged<int> onSelected;

  /// Equal-width tabs filling the row (panels 5 and 6) instead of
  /// content-width pills (panel 2).
  final bool expand;
  @override
  Widget build(BuildContext context) {
    Widget tab(int i) {
      final on = i == selected;
      return Container(
        decoration: BoxDecoration(
            color: on ? PatientTokens.primary : PatientTokens.surface,
            borderRadius: BorderRadius.circular(12),
            boxShadow: on ? null : PatientTokens.softShadow),
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(12),
          child: InkWell(
            onTap: () => onSelected(i),
            borderRadius: BorderRadius.circular(12),
            child: Container(
              constraints: const BoxConstraints(minHeight: 44, minWidth: 64),
              alignment: Alignment.center,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
              child: Text(labels[i],
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                      fontSize: 13,
                      fontWeight: on ? FontWeight.w700 : FontWeight.w600,
                      color: on ? Colors.white : PatientTokens.ink)),
            ),
          ),
        ),
      );
    }

    if (expand) {
      return Row(children: [
        for (var i = 0; i < labels.length; i++) ...[
          if (i > 0) const SizedBox(width: 10),
          Expanded(child: tab(i)),
        ]
      ]);
    }
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      clipBehavior: Clip.none,
      child: Row(children: [
        for (var i = 0; i < labels.length; i++) ...[
          if (i > 0) const SizedBox(width: 10),
          tab(i),
        ]
      ]),
    );
  }
}

/// Soft white card with shadow, the reference's standard row container.
class AfyaCard extends StatelessWidget {
  const AfyaCard(
      {super.key,
      required this.child,
      this.onTap,
      this.color = PatientTokens.surface,
      this.shadow = true,
      this.padding = const EdgeInsets.all(12)});
  final Widget child;
  final VoidCallback? onTap;
  final Color color;
  final bool shadow;
  final EdgeInsets padding;
  @override
  Widget build(BuildContext context) => Container(
        decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(14),
            boxShadow: shadow ? PatientTokens.softShadow : null),
        child: Material(
          color: Colors.transparent,
          borderRadius: BorderRadius.circular(14),
          child: InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(14),
              child: Padding(padding: padding, child: child)),
        ),
      );
}

/// Tinted information card: leading icon, coloured title, body lines.
class AfyaInfoCard extends StatelessWidget {
  const AfyaInfoCard(
      {super.key,
      required this.icon,
      required this.title,
      required this.body,
      this.background = PatientTokens.infoCard,
      this.accent = PatientTokens.infoAccent});
  final IconData icon;
  final String title, body;
  final Color background, accent;
  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 18),
        decoration: BoxDecoration(
            color: background, borderRadius: BorderRadius.circular(14)),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Icon(icon, color: accent, size: 26),
          const SizedBox(width: 14),
          Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                Text(title,
                    style: TextStyle(
                        fontSize: 13.5,
                        fontWeight: FontWeight.w700,
                        color: accent)),
                const SizedBox(height: 6),
                Text(body,
                    style: const TextStyle(
                        fontSize: 12.5, height: 1.6, color: PatientTokens.ink)),
              ])),
        ]),
      );
}

/// Full-width pill call-to-action with a leading icon. A null [onPressed]
/// renders it disabled, used where the feature behind it does not exist yet.
class AfyaPillButton extends StatelessWidget {
  const AfyaPillButton(
      {super.key,
      required this.label,
      required this.icon,
      required this.onPressed,
      this.color = PatientTokens.primary});
  final String label;
  final IconData icon;
  final VoidCallback? onPressed;
  final Color color;
  @override
  Widget build(BuildContext context) => SizedBox(
        width: double.infinity,
        child: FilledButton.icon(
          onPressed: onPressed,
          icon: Icon(icon, size: 20),
          label: Text(label, textAlign: TextAlign.center),
          style: FilledButton.styleFrom(
              backgroundColor: color,
              foregroundColor: Colors.white,
              disabledBackgroundColor: color.withValues(alpha: .35),
              disabledForegroundColor: Colors.white,
              minimumSize: const Size.fromHeight(52),
              shape: const StadiumBorder(),
              textStyle: const TextStyle(
                  fontFamily: 'Plus Jakarta Sans',
                  fontSize: 15,
                  fontWeight: FontWeight.w700)),
        ),
      );
}

/// Green check and a line of text (panels 8 and 11).
class AfyaCheckRow extends StatelessWidget {
  const AfyaCheckRow(this.text, {super.key});
  final String text;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Icon(Icons.check_circle_rounded,
              color: PatientTokens.primary, size: 22),
          const SizedBox(width: 14),
          Expanded(
              child: Text(text,
                  style: const TextStyle(
                      fontSize: 14, height: 1.4, color: PatientTokens.ink))),
        ]),
      );
}

/// Centred note for a tab or section whose feature has no backend yet.
class AfyaUnavailableNote extends StatelessWidget {
  const AfyaUnavailableNote(
      {super.key, required this.icon, this.title, required this.body});
  final IconData icon;
  final String? title;
  final String body;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 36),
        child: Column(mainAxisSize: MainAxisSize.min, children: [
          AfyaIconTile(
              icon: icon,
              color: PatientTokens.navInactive,
              background: PatientTokens.surfaceSubtle,
              size: 64),
          if (title != null) ...[
            const SizedBox(height: 16),
            Text(title!,
                textAlign: TextAlign.center,
                style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.w800,
                    color: PatientTokens.text)),
          ],
          const SizedBox(height: 8),
          Text(body,
              textAlign: TextAlign.center,
              style: const TextStyle(
                  fontSize: 13.5, height: 1.5, color: PatientTokens.textSub)),
        ]),
      );
}

/// Support-chat category tile (reference panel 3): a rounded gradient square
/// with a white glyph, drawn natively so it is sharp at any density.
class AfyaGradientIcon extends StatelessWidget {
  const AfyaGradientIcon(
      {super.key,
      required this.colors,
      this.icon,
      this.ribbon = false,
      this.size = 56});
  final List<Color> colors;
  final IconData? icon;

  /// Draws an awareness ribbon instead of [icon] (no Material glyph exists).
  final bool ribbon;
  final double size;
  @override
  Widget build(BuildContext context) => Container(
        width: size,
        height: size,
        alignment: Alignment.center,
        decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(size * .26),
            gradient: LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: colors)),
        child: ribbon
            ? CustomPaint(
                size: Size.square(size * .56), painter: _RibbonPainter())
            : Icon(icon, color: Colors.white, size: size * .56),
      );
}

class _RibbonPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size s) {
    final p = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = s.width * .17
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    final w = s.width, h = s.height;
    final path = Path()
      ..moveTo(w * .22, h * .96)
      ..lineTo(w * .68, h * .30)
      ..cubicTo(w * .78, h * .12, w * .62, h * .04, w * .5, h * .04)
      ..cubicTo(w * .38, h * .04, w * .22, h * .12, w * .32, h * .30)
      ..lineTo(w * .78, h * .96);
    canvas.drawPath(path, p);
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

/// Hero card for a character illustration (panels 4 and 8): a tinted rounded
/// card with the transparent character standing on its bottom edge. [panel]
/// adds the paler inner rectangle the doctor panel shows behind the figure.
class AfyaCharacterCard extends StatelessWidget {
  const AfyaCharacterCard(this.name,
      {super.key, required this.background, this.panel, this.height = 172});
  final String name;
  final Color background;
  final Color? panel;
  final double height;
  @override
  Widget build(BuildContext context) => Container(
        height: height,
        clipBehavior: Clip.antiAlias,
        decoration: BoxDecoration(
            color: background, borderRadius: BorderRadius.circular(14)),
        child: Stack(alignment: Alignment.bottomCenter, children: [
          if (panel != null)
            Positioned(
                top: 14,
                bottom: 0,
                child: Container(
                    width: height * .92,
                    decoration: BoxDecoration(
                        color: panel,
                        borderRadius: const BorderRadius.vertical(
                            top: Radius.circular(14))))),
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Image.asset('assets/illustrations/$name.png',
                fit: BoxFit.contain,
                alignment: Alignment.bottomCenter,
                filterQuality: FilterQuality.medium,
                excludeFromSemantics: true),
          ),
        ]),
      );
}

/// Illustration asset clipped to a rounded card, clipped to a rounded
/// card. Decorative: excluded from semantics.
class AfyaArt extends StatelessWidget {
  const AfyaArt(this.name,
      {super.key,
      this.width,
      this.height,
      this.radius = 12,
      this.fit = BoxFit.cover,
      this.alignment = Alignment.center});
  final String name;
  final double? width, height;
  final double radius;
  final BoxFit fit;
  final Alignment alignment;
  @override
  Widget build(BuildContext context) => ClipRRect(
        borderRadius: BorderRadius.circular(radius),
        child: Image.asset('assets/illustrations/$name.png',
            width: width ?? (fit == BoxFit.fitWidth ? double.infinity : null),
            height: height,
            fit: fit,
            alignment: alignment,
            filterQuality: FilterQuality.medium,
            excludeFromSemantics: true),
      );
}
