import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/network/api_client.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../../core/widgets/onboarding_language_switch.dart';
import '../../l10n/app_localizations.dart';
import 'account_onboarding_gate.dart';

/// Key of this doctor's "onboarding seen" flag (kept on sign-out).
String doctorOnboardingSeenKey(String account) =>
    onboardingSeenKey('doctor', account);

/// Shows [DoctorOnboardingPage] instead of [child] the first time an
/// approved doctor account opens the app on this phone (first login, or
/// first launch after approval). See [AccountOnboardingGate].
class DoctorOnboardingGate extends StatelessWidget {
  const DoctorOnboardingGate(
      {super.key,
      required this.store,
      required this.account,
      required this.child});
  final TokenStore store;

  /// Identifies the doctor (user id, else licence number); null until the
  /// profile has loaded.
  final String? account;
  final Widget child;
  @override
  Widget build(BuildContext context) => AccountOnboardingGate(
      store: store,
      kind: 'doctor',
      account: account,
      onboarding: (onDone) => DoctorOnboardingPage(onDone: onDone),
      child: child);
}

// ---------------------------------------------------------------------------
// Geometry, in the reference board's own pixels ("AfyaMkononi Doctor
// Onboarding Screens", product owner, 2026-11-20). Each screen is about
// 405 x 941 inside its phone frame. The illustrations and the bottom wave are
// the board's own pixels, saved losslessly. The logo is the app's own
// (product-owner instruction: the board's logo is reference only). All
// words, Skip, the page dots and the buttons are drawn here so they can be
// translated. Positions and colours were sampled from the board.
// ---------------------------------------------------------------------------

const _page = Color(0xFFFEFEFE);
const _title = Color(0xFF072437);
const _body = Color(0xFF2C4054);
const _green = Color(0xFF017047);
const _skip = Color(0xFF077446);
const _secondary = Color(0xFFECF2F3);
const _secondaryText = Color(0xFF0A283D);
const _dotIdle = Color(0xFFCCDDDC);
const _wordmark = Color(0xFF0A645D);
const _tagline = Color(0xFF243746);

const _skipRect = Rect.fromLTWH(311, 53.5, 72, 46);
const _buttonRadius = 14.0;

enum _Action { back, next, finish }

class _Button {
  const _Button(this.rect, this.action, {this.primary = true});
  final Rect rect;
  final _Action action;
  final bool primary;
}

class _Slide {
  const _Slide({
    required this.asset,
    required this.size,
    required this.waveTop,
    required this.titleTop,
    required this.titleSize,
    this.titleShift = 0,
    required this.bodyTop,
    required this.bodyLineHeight,
    required this.dotsY,
    required this.dotsX,
    required this.activeDot,
    required this.buttons,
  });
  final String asset;
  final Size size;

  /// Where the bottom-wave strip asset starts.
  final double waveTop;
  final double titleTop, titleSize;

  /// Moves the centred title right (slide 1 centres title and hand).
  final double titleShift;
  final double bodyTop, bodyLineHeight;
  final double dotsY;
  final List<double> dotsX;
  final Color activeDot;
  final List<_Button> buttons;

  String get waveStrip => asset.replaceFirst('.webp', '_wave_strip.webp');
}

const _bodySize = 22.0;

const _slides = <_Slide>[
  _Slide(
    asset: 'assets/onboarding/doctor_onboarding_1.webp',
    size: Size(405, 941),
    waveTop: 898,
    titleTop: 559,
    titleSize: 31.5,
    titleShift: 7,
    bodyTop: 616,
    bodyLineHeight: 32 / _bodySize,
    dotsY: 773,
    dotsX: [168.5, 192, 216.5, 239.5],
    // The board's slide 1 shows its current dot in a pale green.
    activeDot: Color(0xFF83CCB1),
    buttons: [_Button(Rect.fromLTWH(23, 821, 360, 59), _Action.next)],
  ),
  _Slide(
    asset: 'assets/onboarding/doctor_onboarding_2.webp',
    size: Size(407, 941),
    waveTop: 898,
    titleTop: 539,
    titleSize: 32,
    bodyTop: 595,
    bodyLineHeight: 30.7 / _bodySize,
    dotsY: 773,
    dotsX: [168, 192, 216, 239.5],
    activeDot: Color(0xFF027C4B),
    buttons: [
      _Button(Rect.fromLTWH(23, 823, 174, 59), _Action.back, primary: false),
      _Button(Rect.fromLTWH(210, 823, 174, 59), _Action.next),
    ],
  ),
  _Slide(
    asset: 'assets/onboarding/doctor_onboarding_3.webp',
    size: Size(405, 945),
    waveTop: 885,
    titleTop: 474,
    titleSize: 31.0,
    bodyTop: 531,
    bodyLineHeight: 31 / _bodySize,
    dotsY: 715,
    dotsX: [168, 192, 216.5, 240],
    activeDot: Color(0xFF027C4B),
    buttons: [
      _Button(Rect.fromLTWH(23, 795, 174, 59), _Action.back, primary: false),
      _Button(Rect.fromLTWH(210, 795, 174, 59), _Action.next),
    ],
  ),
  _Slide(
    asset: 'assets/onboarding/doctor_onboarding_4.webp',
    size: Size(407, 945),
    waveTop: 885,
    titleTop: 403,
    titleSize: 31.2,
    bodyTop: 457,
    bodyLineHeight: 30 / _bodySize,
    dotsY: 666,
    dotsX: [167, 191.5, 215, 239],
    activeDot: Color(0xFF027C4B),
    buttons: [
      _Button(Rect.fromLTWH(23, 724, 361, 59), _Action.finish),
      _Button(Rect.fromLTWH(23, 798, 361, 59), _Action.back, primary: false),
    ],
  ),
];

/// How a slide sits on this screen: its scale and top-left. The whole
/// slide always shows (the page is plain white around it); on a phone with
/// a tall navigation bar it is lifted so the buttons stay clear of it.
({double scale, Offset origin, double lift}) _fit(
    Size design, Size screen, EdgeInsets inset, double buttonsBottom) {
  var scale =
      math.min(screen.width / design.width, screen.height / design.height);
  var lift =
      math.max(0.0, inset.bottom + 8 - (design.height - buttonsBottom) * scale);
  if (lift > 0) {
    scale = math.min(
        screen.width / design.width, (screen.height - lift) / design.height);
    lift = math.max(
        0.0, inset.bottom + 8 - (design.height - buttonsBottom) * scale);
  }
  final w = design.width * scale, h = design.height * scale;
  return (
    scale: scale,
    origin: Offset((screen.width - w) / 2, screen.height - lift - h),
    lift: lift,
  );
}

/// Four-slide Doctor onboarding, drawn to the product owner's reference
/// board. Skip leaves at any point; the last button finishes. Both call
/// [onDone].
class DoctorOnboardingPage extends StatefulWidget {
  const DoctorOnboardingPage({super.key, required this.onDone});
  final VoidCallback onDone;
  @override
  State<DoctorOnboardingPage> createState() => _DoctorOnboardingPageState();
}

class _DoctorOnboardingPageState extends State<DoctorOnboardingPage> {
  final _pages = PageController();
  int _index = 0;

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _act(_Action action) {
    switch (action) {
      case _Action.finish:
        widget.onDone();
      case _Action.next:
        _pages.nextPage(
            duration: const Duration(milliseconds: 280), curve: Curves.easeOut);
      case _Action.back:
        _pages.previousPage(
            duration: const Duration(milliseconds: 280), curve: Curves.easeOut);
    }
  }

  @override
  Widget build(BuildContext context) => AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark,
        child: Scaffold(
          backgroundColor: _page,
          body: PageView.builder(
            controller: _pages,
            itemCount: _slides.length,
            onPageChanged: (i) => setState(() => _index = i),
            itemBuilder: (_, i) => _SlideView(
                index: i,
                slide: _slides[i],
                active: i == _index,
                onAction: _act,
                onSkip: widget.onDone),
          ),
        ),
      );
}

class _SlideView extends StatelessWidget {
  const _SlideView(
      {required this.index,
      required this.slide,
      required this.active,
      required this.onAction,
      required this.onSkip});
  final int index;
  final _Slide slide;

  /// Only the slide on screen exposes its buttons (keys and semantics).
  final bool active;
  final ValueChanged<_Action> onAction;
  final VoidCallback onSkip;

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final media = MediaQuery.of(context);
    final buttonsBottom =
        slide.buttons.map((b) => b.rect.bottom).reduce(math.max);
    final fit = _fit(slide.size, media.size, media.viewPadding, buttonsBottom);
    final s = fit.scale, o = fit.origin;
    final (title, body) = switch (index) {
      0 => (l.doctorOnboarding1Title, l.doctorOnboarding1Body),
      1 => (l.doctorOnboarding2Title, l.doctorOnboarding2Body),
      2 => (l.doctorOnboarding3Title, l.doctorOnboarding3Body),
      _ => (l.doctorOnboarding4Title, l.doctorOnboarding4Body),
    };
    final w = slide.size.width;
    return ColoredBox(
      color: _page,
      child: Stack(clipBehavior: Clip.hardEdge, children: [
        Positioned(
          left: o.dx,
          top: o.dy,
          width: w * s,
          height: slide.size.height * s,
          child: FittedBox(
            fit: BoxFit.fill,
            child: SizedBox.fromSize(
              size: slide.size,
              child: MediaQuery.withNoTextScaling(
                child: Stack(children: [
                  Positioned.fill(
                    child: Image.asset(slide.asset,
                        fit: BoxFit.fill,
                        filterQuality: FilterQuality.medium,
                        excludeFromSemantics: true),
                  ),
                  if (index == 0) ...[
                    // The app's own logo in the board's logo box.
                    const Positioned(
                        left: 64, top: 112.5, child: AfyaLogoMark(size: 67)),
                    Positioned(
                      left: 139,
                      top: 116,
                      width: w - 139 - 16,
                      height: 64,
                      child: _Brand(tagline: l.brandTagline),
                    ),
                  ],
                  // EN | SW beside Skip (product owner, 2026-11-21).
                  Positioned(
                    left: 311 - 8 - 92,
                    top: _skipRect.top + 4,
                    width: 92,
                    height: _skipRect.height - 8,
                    child: ExcludeFocus(
                      excluding: !active,
                      child: const OnboardingLanguageSwitch(
                          fill: _secondary,
                          active: _green,
                          text: _secondaryText,
                          fontSize: 15),
                    ),
                  ),
                  Positioned(
                    left: 311,
                    top: _skipRect.top,
                    width: _skipRect.width,
                    height: _skipRect.height,
                    child: _Tap(
                      key: active
                          ? const ValueKey('doctor-onboarding-skip')
                          : null,
                      onTap: onSkip,
                      radius: 12,
                      child: Text(l.doctorOnboardingSkip,
                          maxLines: 1, style: _font(20, 700, _skip, height: 1)),
                    ),
                  ),
                  Positioned(
                    left: 16 + 2 * slide.titleShift,
                    right: 16,
                    top: slide.titleTop,
                    height: slide.bodyTop - slide.titleTop - 2,
                    child: FittedBox(
                      fit: BoxFit.scaleDown,
                      alignment: Alignment.topCenter,
                      child: _TitleText(title,
                          size: slide.titleSize, wave: index == 0),
                    ),
                  ),
                  Positioned(
                    left: 16,
                    right: 16,
                    top: slide.bodyTop,
                    height: slide.dotsY - 18 - slide.bodyTop,
                    child: FittedBox(
                      fit: BoxFit.scaleDown,
                      alignment: Alignment.topCenter,
                      child: Text(body,
                          textAlign: TextAlign.center,
                          style: _font(_bodySize, 500, _body,
                              height: slide.bodyLineHeight)),
                    ),
                  ),
                  Positioned(
                    left: 0,
                    right: 0,
                    top: slide.dotsY - 7,
                    height: 14,
                    child: Semantics(
                      container: true,
                      label:
                          l.doctorOnboardingCounter(index + 1, _slides.length),
                      child: _Dots(slide: slide, index: index),
                    ),
                  ),
                  for (final b in slide.buttons)
                    Positioned.fromRect(
                      rect: b.rect,
                      child: _SlideButton(
                        button: b,
                        active: active,
                        label: switch (b.action) {
                          _Action.back => l.doctorOnboardingBack,
                          _Action.finish => l.doctorOnboardingFinish,
                          _Action.next => index == 0
                              ? l.doctorOnboardingGetStarted
                              : l.doctorOnboardingNext,
                        },
                        onTap: () => onAction(b.action),
                      ),
                    ),
                ]),
              ),
            ),
          ),
        ),
        // The bottom wave spans the full screen width (and fills any space
        // below a lifted slide), drawn over the slide's own copy of it.
        Positioned(
          left: 0,
          right: 0,
          bottom: 0,
          height: (slide.size.height - slide.waveTop) * s + fit.lift,
          child: IgnorePointer(
            child: Image.asset(slide.waveStrip,
                fit: BoxFit.fill,
                filterQuality: FilterQuality.medium,
                excludeFromSemantics: true),
          ),
        ),
      ]),
    );
  }
}

TextStyle _font(double size, int weight, Color color, {double? height}) =>
    TextStyle(
        fontFamily: 'Plus Jakarta Sans',
        fontSize: size,
        height: height,
        fontWeight: FontWeight.values[weight ~/ 100 - 1],
        fontVariations: [FontVariation('wght', weight.toDouble())],
        color: color);

class _TitleText extends StatelessWidget {
  const _TitleText(this.text, {required this.size, required this.wave});
  final String text;
  final double size;
  final bool wave;
  @override
  Widget build(BuildContext context) => Text.rich(
        TextSpan(style: _font(size, 800, _title, height: 1.2), children: [
          TextSpan(text: text),
          if (wave) ...[
            const WidgetSpan(child: SizedBox(width: 9)),
            WidgetSpan(
              alignment: PlaceholderAlignment.middle,
              child: Image.asset(
                  'assets/onboarding/doctor_onboarding_wave.webp',
                  width: 39,
                  height: 40,
                  excludeFromSemantics: true),
            ),
          ],
        ]),
        textAlign: TextAlign.center,
      );
}

/// "AfyaMkononi" and the tagline beside the logo on slide 1.
class _Brand extends StatelessWidget {
  const _Brand({required this.tagline});
  final String tagline;
  @override
  Widget build(BuildContext context) {
    final parts = tagline.split('·').map((p) => p.trim()).toList();
    return FittedBox(
      fit: BoxFit.scaleDown,
      alignment: Alignment.topLeft,
      child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
        Text('AfyaMkononi', style: _font(28.6, 800, _wordmark, height: 1.25)),
        const SizedBox(height: 3),
        Text.rich(TextSpan(style: _font(12.5, 600, _tagline), children: [
          for (var i = 0; i < parts.length; i++) ...[
            if (i > 0) TextSpan(text: ' · ', style: _font(12.5, 700, _tagline)),
            TextSpan(text: parts[i]),
          ],
        ])),
      ]),
    );
  }
}

class _Dots extends StatelessWidget {
  const _Dots({required this.slide, required this.index});
  final _Slide slide;
  final int index;
  @override
  Widget build(BuildContext context) => Stack(children: [
        for (var i = 0; i < slide.dotsX.length; i++)
          Positioned(
            left: slide.dotsX[i] - (i == index ? 7 : 6),
            top: i == index ? 0 : 1,
            width: i == index ? 14 : 12,
            height: i == index ? 14 : 12,
            child: DecoratedBox(
              decoration: BoxDecoration(
                  color: i == index ? slide.activeDot : _dotIdle,
                  shape: BoxShape.circle),
            ),
          ),
      ]);
}

class _SlideButton extends StatelessWidget {
  const _SlideButton(
      {required this.button,
      required this.active,
      required this.label,
      required this.onTap});
  final _Button button;
  final bool active;
  final String label;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final fg = button.primary ? Colors.white : _secondaryText;
    final text =
        Text(label, maxLines: 1, style: _font(22.6, 800, fg, height: 1));
    final key = !active
        ? null
        : ValueKey(switch (button.action) {
            _Action.back => 'doctor-onboarding-back',
            _Action.next => 'doctor-onboarding-next',
            _Action.finish => 'doctor-onboarding-finish',
          });
    return _Tap(
      key: key,
      onTap: onTap,
      color: button.primary ? _green : _secondary,
      radius: _buttonRadius,
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        if (button.action == _Action.back) ...[
          Icon(Icons.arrow_back_rounded, color: fg, size: 26),
          const SizedBox(width: 10),
        ],
        text,
        if (button.action == _Action.next) ...[
          const SizedBox(width: 10),
          Icon(Icons.arrow_forward_rounded, color: fg, size: 26),
        ],
      ]),
    );
  }
}

/// A rounded tap target whose content shrinks rather than overflowing.
class _Tap extends StatelessWidget {
  const _Tap(
      {super.key,
      required this.onTap,
      required this.child,
      required this.radius,
      this.color = Colors.transparent});
  final VoidCallback onTap;
  final Widget child;
  final double radius;
  final Color color;
  @override
  Widget build(BuildContext context) => Material(
        color: color,
        borderRadius: BorderRadius.circular(radius),
        clipBehavior: Clip.antiAlias,
        child: InkWell(
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10),
            child:
                Center(child: FittedBox(fit: BoxFit.scaleDown, child: child)),
          ),
        ),
      );
}
