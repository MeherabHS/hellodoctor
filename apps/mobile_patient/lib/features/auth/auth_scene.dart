import 'package:flutter/material.dart';

/// Size-independent frame for the illustrated auth screens.
///
/// The product owner's designs are 1024 x 1536 illustrations with the form
/// painted into them. Only the artwork above the form is used as a crisp
/// header; the form card is a native widget below it that takes the height
/// of its own content. Nothing is positioned against painted UI, so the same
/// layout holds on any screen size, in landscape, at large text sizes and
/// with the keyboard open:
///
/// - the header scales with the screen width, capped so it never takes more
///   than [maxHeaderFraction] of the screen height;
/// - the card overlaps the header's lower edge, as in the designs;
/// - a heavily blurred copy of the whole artwork fills the rest, giving each
///   screen the design's colours without any painted buttons or text;
/// - the content scrolls whenever it does not fit.
class AuthScene extends StatelessWidget {
  const AuthScene(
      {super.key,
      required this.name,
      required this.headerRows,
      required this.topColor,
      required this.child,
      this.sideMargin = 20,
      this.overlap = 22});

  /// Asset prefix: `assets/auth/<name>_header.webp` and `_bg.webp`.
  final String name;

  /// Height of the header artwork in pixels (it is 1024 wide).
  final double headerRows;

  /// Colour of the artwork's top edge, used behind the status bar.
  final Color topColor;
  final Widget child;
  final double sideMargin, overlap;

  /// Content never grows wider than a large phone.
  static const maxWidth = 480.0;
  static const maxHeaderFraction = .42;

  @override
  Widget build(BuildContext context) =>
      LayoutBuilder(builder: (context, viewport) {
        final inset = MediaQuery.paddingOf(context).top;
        final width = viewport.maxWidth.clamp(0.0, maxWidth);
        // Natural header height at this width, capped for short screens.
        final natural = width * headerRows / 1024;
        final headerHeight = natural
            .clamp(0.0, viewport.maxHeight * maxHeaderFraction)
            .toDouble();
        return Stack(children: [
          Positioned.fill(child: ColoredBox(color: topColor)),
          Positioned.fill(
            child: Image.asset('assets/auth/${name}_bg.webp',
                fit: BoxFit.cover, excludeFromSemantics: true),
          ),
          SingleChildScrollView(
            child: ConstrainedBox(
              constraints: BoxConstraints(minHeight: viewport.maxHeight),
              child: Align(
                alignment: Alignment.topCenter,
                child: SizedBox(
                  width: width,
                  child: Stack(children: [
                    Positioned(
                      top: 0,
                      left: 0,
                      right: 0,
                      height: inset + headerHeight,
                      child: Column(children: [
                        Container(height: inset, color: topColor),
                        Expanded(child: _header()),
                      ]),
                    ),
                    Padding(
                      padding: EdgeInsets.fromLTRB(sideMargin,
                          inset + headerHeight - overlap, sideMargin, 24),
                      child: child,
                    ),
                  ]),
                ),
              ),
            ),
          ),
        ]);
      });

  /// The header fades out along its bottom edge so it blends into the
  /// ambience layer at the sides of the card instead of ending in a line.
  Widget _header() => ShaderMask(
        blendMode: BlendMode.dstIn,
        shaderCallback: (rect) => const LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [Colors.white, Colors.white, Colors.transparent],
            stops: [0, .86, 1]).createShader(rect),
        child: Image.asset('assets/auth/${name}_header.webp',
            width: double.infinity,
            fit: BoxFit.cover,
            alignment: Alignment.topCenter,
            filterQuality: FilterQuality.medium,
            excludeFromSemantics: true),
      );
}

/// Colours of the auth designs, sampled from the supplied artwork.
abstract final class AuthColors {
  static const title = Color(0xFF0B3D33);
  static const label = Color(0xFF0A4A3C);
  static const body = Color(0xFF3C5A52);
  static const link = Color(0xFF0A7D5F);
  static const buttonTop = Color(0xFF14B588);
  static const buttonBottom = Color(0xFF0A8F6A);
  static const darkCard = Color(0xFF065041);
  static const darkButton = Color(0xFF0B5A45);
  static const cream = Color(0xFFFEF8EB);
  static const fieldBorder = Color(0xFFC9D2CC);
  static const lockTile = Color(0xFFD9ECDC);
  static const resetButton = Color(0xFF0FA97A);
}

/// Primary pill button of the auth designs. It wraps a [FilledButton] so
/// tests and semantics treat it as the screen's primary action.
class AuthPrimaryButton extends StatelessWidget {
  const AuthPrimaryButton(
      {super.key,
      required this.label,
      required this.onPressed,
      this.icon,
      this.trailingIcon,
      this.busy = false,
      this.gradient,
      this.color = AuthColors.darkButton});
  final String label;
  final VoidCallback? onPressed;
  final IconData? icon, trailingIcon;
  final bool busy;
  final List<Color>? gradient;
  final Color color;
  @override
  Widget build(BuildContext context) => DecoratedBox(
        decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(26),
            color: gradient == null ? color : null,
            gradient: gradient == null
                ? null
                : LinearGradient(
                    begin: Alignment.topCenter,
                    end: Alignment.bottomCenter,
                    colors: gradient!),
            boxShadow: const [
              BoxShadow(
                  color: Color(0x330A6B50),
                  blurRadius: 10,
                  offset: Offset(0, 4))
            ]),
        child: FilledButton(
          onPressed: onPressed,
          style: FilledButton.styleFrom(
              backgroundColor: Colors.transparent,
              disabledBackgroundColor: Colors.transparent,
              foregroundColor: Colors.white,
              disabledForegroundColor: Colors.white70,
              shadowColor: Colors.transparent,
              minimumSize: const Size.fromHeight(52),
              shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(26)),
              textStyle: const TextStyle(
                  fontFamily: 'Plus Jakarta Sans',
                  fontSize: 16,
                  fontWeight: FontWeight.w700)),
          child: busy
              ? const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(
                      strokeWidth: 2, color: Colors.white))
              : Row(mainAxisSize: MainAxisSize.min, children: [
                  if (icon != null) ...[
                    Icon(icon, size: 22),
                    const SizedBox(width: 10),
                  ],
                  Flexible(child: Text(label, textAlign: TextAlign.center)),
                  if (trailingIcon != null) ...[
                    const SizedBox(width: 10),
                    Icon(trailingIcon, size: 22),
                  ],
                ]),
        ),
      );
}
