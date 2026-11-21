import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/network/api_client.dart';
import '../../core/widgets/afya_widgets.dart';
import '../../core/widgets/onboarding_language_switch.dart';
import '../../l10n/app_localizations.dart';

// The flag's prefix lives with the store that keeps it across sign-out.
export '../../core/network/api_client.dart' show onboardingSeenKey;

/// The "seen" flag for the signed-in account: one per patient account on
/// this phone, read from the access token's subject. A token without one
/// falls back to the phone-wide flag.
String onboardingSeenKeyFor(String? accessToken) {
  final parts = accessToken?.split('.') ?? const <String>[];
  if (parts.length == 3) {
    try {
      final payload = jsonDecode(
          utf8.decode(base64Url.decode(base64Url.normalize(parts[1]))));
      if (payload is Map) {
        for (final key in const ['sub', 'patient_id', 'user_id']) {
          final id = payload[key]?.toString() ?? '';
          if (id.isNotEmpty) return '$onboardingSeenKey:$id';
        }
      }
    } catch (_) {/* opaque token: phone-wide flag */}
  }
  return onboardingSeenKey;
}

/// Shows [OnboardingPage] right after sign-in (or after creating an
/// account), once per patient account on this phone, then [child] (the
/// signed-in app). Product owner, 2026-11-21: onboarding follows sign-in in
/// both apps. A storage error never blocks the app: it is treated as
/// "already seen".
class OnboardingGate extends StatefulWidget {
  const OnboardingGate({super.key, required this.store, required this.child});
  final TokenStore store;
  final Widget child;
  @override
  State<OnboardingGate> createState() => _OnboardingGateState();
}

class _OnboardingGateState extends State<OnboardingGate> {
  bool? _seen;
  String _key = onboardingSeenKey;

  @override
  void initState() {
    super.initState();
    _check();
  }

  Future<void> _check() async {
    try {
      _key = onboardingSeenKeyFor(await widget.store.read('access_token'));
      final value = await widget.store.read(_key);
      _set(value != null);
    } catch (_) {
      _set(true);
    }
  }

  void _set(bool seen) {
    if (mounted) setState(() => _seen = seen);
  }

  Future<void> _finish() async {
    _set(true);
    try {
      await widget.store.write(_key, '1');
    } catch (_) {/* shown again next launch at worst */}
  }

  @override
  Widget build(BuildContext context) => switch (_seen) {
        null => const Scaffold(backgroundColor: Colors.white),
        true => widget.child,
        false => OnboardingPage(onDone: _finish),
      };
}

// ---------------------------------------------------------------------------
// Geometry. Everything below is in the reference board's own pixels: each
// screen of "AfyaMkononi Youth Health Onboarding" (product owner, 2026-11-20)
// is 430 x 882 inside the phone frame. The artwork (characters, Kenyan
// pattern, plants, icon circles) is the board's own pixels, saved
// losslessly; the words, Skip, page dots and button are drawn here so they
// can be translated. Positions and colours were sampled from the board.
// ---------------------------------------------------------------------------

const _designW = 430.0, _designH = 882.0;

/// At most this share of the artwork's width may be cropped (half per side)
/// on phones taller than the reference before the artwork stops growing and
/// the space above it is filled instead.
const _maxSideCrop = 0.06;

const _titleColor = Color(0xFF0A1E34);
const _bodyColor = Color(0xFF142034);
const _skipText = Color(0xFF113C55);
const _dotActive = Color(0xFF045541);
const _dotIdle = Color(0xFFC4C8CC);
const _wordmark = Color(0xFF04536A);
const _taglineText = Color(0xFF062942);
const _taglineDot = Color(0xFF07864B);

class _Slide {
  const _Slide({
    required this.asset,
    required this.top,
    required this.skipFill,
    required this.button,
    required this.title,
    required this.body,
  });
  final String asset;

  /// The artwork's top-row colour, used above it on very tall phones.
  final Color top;
  final Color skipFill;
  final Color button;
  final _TextBox title;
  final _TextBox body;
}

/// A text block: [left]/[top] of the line box, the [right] edge it must not
/// pass, the [bottom] it must not pass, and its type size and line height.
class _TextBox {
  const _TextBox(
      {required this.left,
      required this.top,
      required this.right,
      required this.bottom,
      required this.size,
      required this.lineHeight,
      this.centered = false});
  final double left, top, right, bottom, size, lineHeight;
  final bool centered;
}

const _titleBottomGap = 4.0;

const _slides = <_Slide>[
  _Slide(
    asset: 'assets/onboarding/onboarding_1.webp',
    top: Color(0xFFFDFEFE),
    skipFill: Color(0xFFE3EEF0),
    button: Color(0xFF01825D),
    title: _TextBox(
        left: 80,
        top: 490,
        right: 420,
        bottom: 584,
        size: 33,
        lineHeight: 1.4,
        centered: true),
    body: _TextBox(
        left: 125,
        top: 589,
        right: 418,
        bottom: 700,
        size: 20,
        lineHeight: 1.28),
  ),
  _Slide(
    asset: 'assets/onboarding/onboarding_2.webp',
    top: Color(0xFFFEFEFE),
    skipFill: Color(0xFFE0EDEE),
    button: Color(0xFF017F5A),
    title: _TextBox(
        left: 128,
        top: 520,
        right: 420,
        bottom: 562,
        size: 27.0,
        lineHeight: 1.2),
    body: _TextBox(
        left: 129,
        top: 568,
        right: 420,
        bottom: 690,
        size: 20,
        lineHeight: 1.3),
  ),
  _Slide(
    asset: 'assets/onboarding/onboarding_3.webp',
    top: Color(0xFFFEFEFE),
    skipFill: Color(0xFFE1EEEF),
    button: Color(0xFF017B57),
    title: _TextBox(
        left: 108,
        top: 517,
        right: 420,
        bottom: 556,
        size: 23.3,
        lineHeight: 1.2),
    body: _TextBox(
        left: 129,
        top: 561,
        right: 420,
        bottom: 700,
        size: 20,
        lineHeight: 1.235),
  ),
  _Slide(
    asset: 'assets/onboarding/onboarding_4.webp',
    top: Color(0xFFFEFEFE),
    skipFill: Color(0xFFE7EEF6),
    button: Color(0xFF01704F),
    title: _TextBox(
        left: 115,
        top: 517,
        right: 420,
        bottom: 560,
        size: 25.7,
        lineHeight: 1.2),
    body: _TextBox(
        left: 129,
        top: 566,
        right: 420,
        bottom: 690,
        size: 20,
        lineHeight: 1.28),
  ),
];

// Fixed parts, in board pixels.
const _skipRect = Rect.fromLTWH(338, 54, 74, 38);
const _buttonRect = Rect.fromLTWH(43, 772, 344, 54);
const _dotCentreY = 737.0, _dotFirstX = 180.0, _dotPitch = 23.0;
const _dotActiveSize = 14.0, _dotIdleSize = 11.0;

/// How the 430 x 882 artwork sits on this screen: its scale and top-left.
({double scale, Offset origin}) _fit(Size screen, EdgeInsets inset) {
  final aspect = screen.width / screen.height;
  double scale;
  if (aspect >= _designW / _designH) {
    // Wider than the board (tablets): whole artwork, centred.
    scale = screen.height / _designH;
  } else {
    scale = math.min(screen.height / _designH,
        screen.width / (_designW * (1 - _maxSideCrop)));
  }
  // Keep the button clear of a tall navigation bar.
  final below = (_designH - _buttonRect.bottom) * scale;
  final lift = math.max(0.0, inset.bottom + 8 - below);
  final w = _designW * scale, h = _designH * scale;
  return (
    scale: scale,
    origin: Offset((screen.width - w) / 2, screen.height - lift - h),
  );
}

/// Four-slide patient onboarding, drawn to the product owner's reference
/// board pixel for pixel. Skip leaves at any point; the last button
/// finishes. Both call [onDone].
class OnboardingPage extends StatefulWidget {
  const OnboardingPage({super.key, required this.onDone});
  final VoidCallback onDone;
  @override
  State<OnboardingPage> createState() => _OnboardingPageState();
}

class _OnboardingPageState extends State<OnboardingPage> {
  final _pages = PageController();
  int _index = 0;

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _next() {
    if (_index == _slides.length - 1) {
      widget.onDone();
      return;
    }
    _pages.nextPage(
        duration: const Duration(milliseconds: 280), curve: Curves.easeOut);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final media = MediaQuery.of(context);
    final fit = _fit(media.size, media.viewPadding);
    final s = fit.scale;
    final o = fit.origin;
    final slide = _slides[_index];
    final last = _index == _slides.length - 1;

    // Board rectangle -> screen rectangle.
    Rect onScreen(Rect r) => Rect.fromLTWH(
        o.dx + r.left * s, o.dy + r.top * s, r.width * s, r.height * s);

    // Skip keeps the board's margin from the screen's own right edge (the
    // artwork's sides may be cropped) and stays below the status bar.
    final skipW = _skipRect.width * s, skipH = _skipRect.height * s;
    final skipTop =
        math.max(o.dy + _skipRect.top * s, media.viewPadding.top + 6);
    final skipRight = (_designW - _skipRect.right) * s;
    final button = onScreen(_buttonRect);

    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: SystemUiOverlayStyle.dark,
      child: Scaffold(
        backgroundColor: slide.top,
        body: Stack(children: [
          Positioned.fill(
            child: PageView.builder(
              controller: _pages,
              itemCount: _slides.length,
              onPageChanged: (i) => setState(() => _index = i),
              itemBuilder: (_, i) =>
                  _SlideView(index: i, slide: _slides[i], scale: s, origin: o),
            ),
          ),
          // Page dots.
          Positioned(
            left: o.dx,
            top: o.dy + (_dotCentreY - _dotActiveSize / 2) * s,
            width: _designW * s,
            height: _dotActiveSize * s,
            child: Semantics(
              label: l.onboardingCounter(_index + 1, _slides.length),
              child: _Dots(index: _index, scale: s),
            ),
          ),
          // EN | SW beside Skip (product owner, 2026-11-21); slimmer than
          // Skip and level with its top so it clears slide 1's wordmark.
          Positioned(
            right: skipRight + skipW + 8 * s,
            top: skipTop,
            width: 80 * s,
            height: 28 * s,
            child: OnboardingLanguageSwitch(
                fill: slide.skipFill,
                active: slide.button,
                text: _skipText,
                fontSize: 13.5 * s),
          ),
          Positioned(
            right: skipRight,
            top: skipTop,
            width: skipW,
            height: skipH,
            child: _Pill(
              key: const ValueKey('onboarding-skip'),
              color: slide.skipFill,
              onTap: widget.onDone,
              child: Text(l.onboardingSkip,
                  maxLines: 1,
                  style: TextStyle(
                      fontSize: 16.8 * s,
                      fontWeight: FontWeight.w600,
                      fontVariations: const [FontVariation('wght', 600)],
                      height: 1,
                      color: _skipText)),
            ),
          ),
          Positioned.fromRect(
            rect: button,
            child: _Pill(
              key: const ValueKey('onboarding-next'),
              color: slide.button,
              onTap: _next,
              // The board's label sits 7 px right of the button's centre.
              child: Padding(
                padding: EdgeInsets.only(left: 15 * s),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  Text(last ? l.onboardingGetStarted : l.onboardingNext,
                      maxLines: 1,
                      style: TextStyle(
                          fontSize: (last ? 23 : 23.6) * s,
                          fontWeight: FontWeight.w700,
                          fontVariations: const [FontVariation('wght', 700)],
                          height: 1,
                          color: Colors.white)),
                  SizedBox(width: (last ? 10 : 12) * s),
                  Icon(last ? Icons.check_rounded : Icons.arrow_forward_rounded,
                      color: Colors.white, size: (last ? 30 : 33) * s),
                ]),
              ),
            ),
          ),
        ]),
      ),
    );
  }
}

/// A pill-shaped tap target whose label shrinks rather than overflowing.
class _Pill extends StatelessWidget {
  const _Pill(
      {super.key,
      required this.color,
      required this.onTap,
      required this.child});
  final Color color;
  final VoidCallback onTap;
  final Widget child;
  @override
  Widget build(BuildContext context) => Material(
        color: color,
        shape: const StadiumBorder(),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8),
            child:
                Center(child: FittedBox(fit: BoxFit.scaleDown, child: child)),
          ),
        ),
      );
}

class _SlideView extends StatelessWidget {
  const _SlideView(
      {required this.index,
      required this.slide,
      required this.scale,
      required this.origin});
  final int index;
  final _Slide slide;
  final double scale;
  final Offset origin;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    // On phones taller than the board the artwork's sides are cropped a
    // little; text must stay inside what is visible.
    final cropped = math.max(0.0, -origin.dx / scale);
    final maxRight = _designW - cropped - 8;
    final (title, body) = switch (index) {
      0 => (l.onboarding1Title, l.onboarding1Body),
      1 => (l.onboarding2Title, l.onboarding2Body),
      2 => (l.onboarding3Title, l.onboarding3Body),
      _ => (l.onboarding4Title, l.onboarding4Body),
    };
    return ColoredBox(
      color: slide.top,
      child: Stack(clipBehavior: Clip.hardEdge, children: [
        Positioned(
          left: origin.dx,
          top: origin.dy,
          width: _designW * scale,
          height: _designH * scale,
          // The text is laid out at board size and scaled with the artwork,
          // so line breaks and spacing stay exactly as on the board.
          child: FittedBox(
            fit: BoxFit.fill,
            child: SizedBox(
              width: _designW,
              height: _designH,
              child: MediaQuery.withNoTextScaling(
                child: Stack(children: [
                  Positioned.fill(
                    child: Image.asset(slide.asset,
                        fit: BoxFit.fill,
                        filterQuality: FilterQuality.medium,
                        excludeFromSemantics: true),
                  ),
                  // The app's own logo, not the board's (product-owner
                  // instruction, 2026-11-20), in the board's logo box.
                  if (index == 0)
                    const Positioned(
                        left: 45, top: 67.5, child: AfyaLogoMark(size: 77)),
                  if (index == 0) const _Brand(),
                  _Block(
                      box: slide.title,
                      maxRight: maxRight,
                      bottomGap: _titleBottomGap,
                      child: _TitleText(title,
                          box: slide.title, wave: index == 0)),
                  _Block(
                      box: slide.body,
                      maxRight: maxRight,
                      child: Text(body,
                          textAlign: TextAlign.start,
                          style: TextStyle(
                              fontSize: slide.body.size,
                              height: slide.body.lineHeight,
                              fontWeight: FontWeight.w500,
                              fontVariations: const [
                                FontVariation('wght', 500)
                              ],
                              color: _bodyColor))),
                ]),
              ),
            ),
          ),
        ),
      ]),
    );
  }
}

/// Places [child] in [box]; a longer translation shrinks to fit rather than
/// overflowing the card.
class _Block extends StatelessWidget {
  const _Block(
      {required this.box,
      required this.child,
      required this.maxRight,
      this.bottomGap = 0});
  final _TextBox box;
  final Widget child;
  final double maxRight;
  final double bottomGap;
  @override
  Widget build(BuildContext context) => Positioned(
        left: box.left,
        top: box.top,
        width: math.min(box.right, maxRight) - box.left,
        height: box.bottom - box.top - bottomGap,
        child: FittedBox(
          fit: BoxFit.scaleDown,
          alignment: box.centered ? Alignment.topCenter : Alignment.topLeft,
          child: child,
        ),
      );
}

class _TitleText extends StatelessWidget {
  const _TitleText(this.text, {required this.box, required this.wave});
  final String text;
  final _TextBox box;
  final bool wave;
  @override
  Widget build(BuildContext context) {
    final style = TextStyle(
        fontSize: box.size,
        height: box.lineHeight,
        fontWeight: FontWeight.w800,
        fontVariations: const [FontVariation('wght', 800)],
        color: _titleColor);
    return Text.rich(
      TextSpan(style: style, children: [
        TextSpan(text: text),
        if (wave) ...[
          const WidgetSpan(child: SizedBox(width: 10)),
          WidgetSpan(
            alignment: PlaceholderAlignment.middle,
            child: Image.asset('assets/onboarding/onboarding_wave.webp',
                width: 54, height: 50, excludeFromSemantics: true),
          ),
        ],
      ]),
      textAlign: box.centered ? TextAlign.center : TextAlign.start,
    );
  }
}

/// Slide 1's wordmark and tagline beside the logo.
class _Brand extends StatelessWidget {
  const _Brand();
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final parts = l.brandTagline.split('•').map((p) => p.trim()).toList();
    return Positioned(
      left: 130,
      top: 76,
      width: 290,
      height: 68,
      child: FittedBox(
        fit: BoxFit.scaleDown,
        alignment: Alignment.topLeft,
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          const Text('AfyaMkononi',
              style: TextStyle(
                  fontSize: 29.8,
                  height: 1.2,
                  fontWeight: FontWeight.w800,
                  fontVariations: [FontVariation('wght', 800)],
                  color: _wordmark)),
          const SizedBox(height: 6),
          Text.rich(TextSpan(
            style: const TextStyle(
                fontSize: 15.3,
                height: 1.2,
                fontWeight: FontWeight.w500,
                fontVariations: [FontVariation('wght', 500)],
                color: _taglineText),
            children: [
              for (var i = 0; i < parts.length; i++) ...[
                if (i > 0)
                  const TextSpan(
                      text: ' • ', style: TextStyle(color: _taglineDot)),
                TextSpan(text: parts[i]),
              ],
            ],
          )),
        ]),
      ),
    );
  }
}

class _Dots extends StatelessWidget {
  const _Dots({required this.index, required this.scale});
  final int index;
  final double scale;
  @override
  Widget build(BuildContext context) => Stack(children: [
        for (var i = 0; i < _slides.length; i++)
          Builder(builder: (context) {
            final d = (i == index ? _dotActiveSize : _dotIdleSize) * scale;
            final cx = (_dotFirstX + i * _dotPitch) * scale;
            return Positioned(
              left: cx - d / 2,
              top: (_dotActiveSize * scale - d) / 2,
              width: d,
              height: d,
              child: AnimatedContainer(
                duration: const Duration(milliseconds: 200),
                decoration: BoxDecoration(
                    color: i == index ? _dotActive : _dotIdle,
                    shape: BoxShape.circle),
              ),
            );
          }),
      ]);
}
