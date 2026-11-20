import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/network/api_client.dart';
import '../../core/widgets/afya_auth_widgets.dart';
import '../../l10n/app_localizations.dart';
import '../peer/peer_ui.dart';
import 'account_onboarding_gate.dart';

/// Shows [PeerOnboardingPage] instead of [child] the first time an approved
/// Peer Navigator account opens the app on this phone (first sign-in, or
/// first launch after approval). See [AccountOnboardingGate].
class PeerOnboardingGate extends StatelessWidget {
  const PeerOnboardingGate(
      {super.key,
      required this.store,
      required this.account,
      required this.child});
  final TokenStore store;
  final String? account;
  final Widget child;
  @override
  Widget build(BuildContext context) => AccountOnboardingGate(
      store: store,
      kind: 'peer',
      account: account,
      onboarding: (onDone) => PeerOnboardingPage(onDone: onDone),
      child: child);
}

// The four Peer Navigator reference screens (product owner, 2026-11-20)
// are references: the illustrations are theirs (speech-bubble lettering
// erased and drawn here), while the header uses the app's own logo, the
// screens share one layout and the wording addresses an approved Peer
// Navigator and names only what the peer app really does (Support Requests,
// Chats, Schedule & Availability).

const _page = Color(0xFFFEF9F4);
const _card = Color(0xFFFFFDFA);
const _wordmark = Color(0xFF0E4A4A);
const _dotIdle = Color(0xFFCFE6DA);

class _Art {
  const _Art(this.asset, this.size, this.text, {this.align = TextAlign.left});
  final String asset;
  final Size size;

  /// Where the bubble's text sits, as fractions of the image.
  final (double, double, double, double) text;
  final TextAlign align;
}

// Text boxes measured from the original lettering on each board.
const _art = [
  _Art('assets/peer/onboarding_peer_1.webp', Size(864, 683),
      (0.548, 0.165, 0.895, 0.425)),
  _Art('assets/peer/onboarding_peer_2.webp', Size(864, 778),
      (0.285, 0.110, 0.690, 0.390),
      align: TextAlign.center),
  _Art('assets/peer/onboarding_peer_3.webp', Size(864, 485),
      (0.462, 0.165, 0.875, 0.495),
      align: TextAlign.center),
  _Art('assets/peer/onboarding_peer_4.webp', Size(852, 605),
      (0.510, 0.085, 0.925, 0.560)),
];

/// Four-slide Peer Navigator onboarding. Skip leaves at any point; the last
/// button finishes. Both call [onDone].
class PeerOnboardingPage extends StatefulWidget {
  const PeerOnboardingPage({super.key, required this.onDone});
  final VoidCallback onDone;
  @override
  State<PeerOnboardingPage> createState() => _PeerOnboardingPageState();
}

class _PeerOnboardingPageState extends State<PeerOnboardingPage> {
  final _pages = PageController();
  int _index = 0;

  @override
  void dispose() {
    _pages.dispose();
    super.dispose();
  }

  void _go(int delta) => delta > 0
      ? _pages.nextPage(
          duration: const Duration(milliseconds: 280), curve: Curves.easeOut)
      : _pages.previousPage(
          duration: const Duration(milliseconds: 280), curve: Curves.easeOut);

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final last = _index == _art.length - 1;
    return AnnotatedRegion<SystemUiOverlayStyle>(
      value: SystemUiOverlayStyle.dark,
      child: Scaffold(
        backgroundColor: _page,
        body: SafeArea(
          child: Column(children: [
            _Header(onSkip: widget.onDone),
            Expanded(
              child: PageView.builder(
                controller: _pages,
                itemCount: _art.length,
                onPageChanged: (i) => setState(() => _index = i),
                itemBuilder: (_, i) => _Slide(index: i),
              ),
            ),
            const SizedBox(height: 10),
            Semantics(
              container: true,
              label: l.peerOnboardingCounter(_index + 1, _art.length),
              child: _Dots(index: _index, count: _art.length),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 14, 20, 14),
              child: Row(children: [
                if (_index > 0) ...[
                  SizedBox(
                    width: 56,
                    height: 56,
                    child: Material(
                      color: PeerTokens.mint,
                      shape: const CircleBorder(),
                      child: InkWell(
                        key: const ValueKey('peer-onboarding-back'),
                        customBorder: const CircleBorder(),
                        onTap: () => _go(-1),
                        child: Tooltip(
                          message: l.peerOnboardingBack,
                          child: const Icon(Icons.arrow_back_rounded,
                              color: PeerTokens.green, size: 28),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 14),
                ],
                Expanded(
                  child: SizedBox(
                    height: 56,
                    child: FilledButton(
                      key: ValueKey(last
                          ? 'peer-onboarding-finish'
                          : 'peer-onboarding-next'),
                      style: FilledButton.styleFrom(
                          backgroundColor: PeerTokens.green,
                          foregroundColor: Colors.white,
                          shape: RoundedRectangleBorder(
                              borderRadius: BorderRadius.circular(18))),
                      onPressed: last ? widget.onDone : () => _go(1),
                      child: FittedBox(
                        fit: BoxFit.scaleDown,
                        child: Row(mainAxisSize: MainAxisSize.min, children: [
                          Text(
                              last
                                  ? l.peerOnboardingFinish
                                  : _index == 0
                                      ? l.peerOnboardingGetStarted
                                      : l.peerOnboardingNext,
                              style: const TextStyle(
                                  fontSize: 18.5,
                                  fontWeight: FontWeight.w800,
                                  fontVariations: [
                                    FontVariation('wght', 800)
                                  ])),
                          const SizedBox(width: 10),
                          const Icon(Icons.arrow_forward_rounded, size: 24),
                        ]),
                      ),
                    ),
                  ),
                ),
              ]),
            ),
          ]),
        ),
      ),
    );
  }
}

class _Header extends StatelessWidget {
  const _Header({required this.onSkip});
  final VoidCallback onSkip;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 10, 14, 8),
      child: Row(children: [
        const AfyaLogoMark(size: 46),
        const SizedBox(width: 8),
        Expanded(
          child: FittedBox(
            fit: BoxFit.scaleDown,
            alignment: Alignment.centerLeft,
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              const Text('AfyaMkononi',
                  style: TextStyle(
                      fontSize: 23,
                      height: 1.15,
                      fontWeight: FontWeight.w800,
                      fontVariations: [FontVariation('wght', 800)],
                      color: _wordmark)),
              Text(l.brandTagline,
                  style: const TextStyle(
                      fontSize: 12.5,
                      fontWeight: FontWeight.w600,
                      fontVariations: [FontVariation('wght', 600)],
                      color: _wordmark)),
            ]),
          ),
        ),
        const SizedBox(width: 8),
        Material(
          color: PeerTokens.mint,
          shape: const StadiumBorder(),
          child: InkWell(
            key: const ValueKey('peer-onboarding-skip'),
            customBorder: const StadiumBorder(),
            onTap: onSkip,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 9),
              child: Text(l.peerOnboardingSkip,
                  style: const TextStyle(
                      fontSize: 16,
                      fontWeight: FontWeight.w800,
                      fontVariations: [FontVariation('wght', 800)],
                      color: PeerTokens.green)),
            ),
          ),
        ),
      ]),
    );
  }
}

class _Slide extends StatelessWidget {
  const _Slide({required this.index});
  final int index;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final art = _art[index];
    final bubble = [
      l.peerOnboarding1Bubble,
      l.peerOnboarding2Bubble,
      l.peerOnboarding3Bubble,
      l.peerOnboarding4Bubble,
    ][index];
    final content = switch (index) {
      0 => _Content(
          title: l.peerOnboarding1Title,
          body: l.peerOnboarding1Body,
          below: _Tiles(tiles: [
            (
              Icons.groups_rounded,
              PeerTokens.peach,
              const Color(0xFFD9822B),
              l.peerOnboarding1Tile1
            ),
            (
              Icons.health_and_safety_rounded,
              PeerTokens.mint,
              PeerTokens.green,
              l.peerOnboarding1Tile2
            ),
            (
              Icons.favorite_border_rounded,
              PeerTokens.redSoft,
              const Color(0xFFE0445A),
              l.peerOnboarding1Tile3
            ),
          ])),
      1 => _Content(
          icon: Icons.groups_rounded,
          title: l.peerOnboarding2Title,
          body: l.peerOnboarding2Body),
      2 => _Content(
          title: l.peerOnboarding3Title,
          body: l.peerOnboarding3Body,
          below: _Rows(rows: [
            // The same icons as the peer app's bottom bar.
            (
              Icons.mail_rounded,
              PeerTokens.redSoft,
              const Color(0xFFE0445A),
              l.peerOnboarding3Row1Title,
              l.peerOnboarding3Row1Body
            ),
            (
              Icons.chat_bubble_rounded,
              PeerTokens.mint,
              PeerTokens.green,
              l.peerOnboarding3Row2Title,
              l.peerOnboarding3Row2Body
            ),
            (
              Icons.calendar_month_rounded,
              PeerTokens.amber,
              PeerTokens.amberText,
              l.peerOnboarding3Row3Title,
              l.peerOnboarding3Row3Body
            ),
          ])),
      _ => _Content(
          above: _Rows(rows: [
            (
              Icons.handshake_outlined,
              PeerTokens.mint,
              PeerTokens.green,
              l.peerOnboarding4Row1Title,
              l.peerOnboarding4Row1Body
            ),
            (
              Icons.groups_outlined,
              PeerTokens.mint,
              PeerTokens.green,
              l.peerOnboarding4Row2Title,
              l.peerOnboarding4Row2Body
            ),
            (
              Icons.shield_outlined,
              PeerTokens.mint,
              PeerTokens.green,
              l.peerOnboarding4Row3Title,
              l.peerOnboarding4Row3Body
            ),
          ]),
          title: l.peerOnboarding4Title,
          body: l.peerOnboarding4Body),
    };
    return LayoutBuilder(builder: (context, box) {
      final width = box.maxWidth;
      // The illustration takes what the content leaves, up to its natural
      // size; the content card overlaps its lower edge.
      return Column(children: [
        Expanded(
          child: LayoutBuilder(builder: (context, area) {
            // Sits on the content card (which overlaps its lower edge);
            // any spare height goes above it, never as a gap below.
            final scale = (area.maxWidth / art.size.width)
                .clamp(0.0, area.maxHeight / art.size.height);
            final w = art.size.width * scale, h = art.size.height * scale;
            final (x0, y0, x1, y1) = art.text;
            return OverflowBox(
              alignment: Alignment.bottomCenter,
              minHeight: 0,
              maxHeight: h,
              child: SizedBox(
                width: w,
                height: h,
                child: Stack(children: [
                  Positioned.fill(
                    child: Image.asset(art.asset,
                        fit: BoxFit.fill,
                        filterQuality: FilterQuality.medium,
                        excludeFromSemantics: true),
                  ),
                  Positioned(
                    left: x0 * w,
                    top: y0 * h,
                    width: (x1 - x0) * w,
                    height: (y1 - y0) * h,
                    child: FittedBox(
                      fit: BoxFit.scaleDown,
                      alignment: art.align == TextAlign.center
                          ? Alignment.center
                          : Alignment.centerLeft,
                      child: Text(bubble,
                          textAlign: art.align,
                          style: TextStyle(
                              fontSize: 34 * scale,
                              height: 1.2,
                              fontWeight: FontWeight.w800,
                              fontVariations: const [
                                FontVariation('wght', 800)
                              ],
                              color: PeerTokens.navy)),
                    ),
                  ),
                ]),
              ),
            );
          }),
        ),
        // On short screens the card shrinks to fit instead of overflowing,
        // always leaving the illustration part of the height.
        ConstrainedBox(
          constraints: BoxConstraints(maxHeight: box.maxHeight * 0.64),
          child: Transform.translate(
            offset: const Offset(0, -28),
            child: FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.topCenter,
              child: Container(
                width: width - 24,
                margin: const EdgeInsets.symmetric(horizontal: 12),
                padding: const EdgeInsets.fromLTRB(16, 18, 16, 6),
                decoration: BoxDecoration(
                  color: _card,
                  borderRadius: BorderRadius.circular(26),
                  boxShadow: const [
                    BoxShadow(
                        color: Color(0x14000000),
                        blurRadius: 14,
                        offset: Offset(0, -2))
                  ],
                ),
                child: content,
              ),
            ),
          ),
        ),
      ]);
    });
  }
}

class _Content extends StatelessWidget {
  const _Content(
      {required this.title,
      required this.body,
      this.icon,
      this.above,
      this.below});
  final String title, body;
  final IconData? icon;
  final Widget? above, below;
  @override
  Widget build(BuildContext context) =>
      Column(mainAxisSize: MainAxisSize.min, children: [
        if (above != null) ...[above!, const SizedBox(height: 14)],
        if (icon != null) ...[
          Container(
            width: 64,
            height: 64,
            decoration: const BoxDecoration(
                color: PeerTokens.mint, shape: BoxShape.circle),
            child: Icon(icon, size: 34, color: PeerTokens.green),
          ),
          const SizedBox(height: 12),
        ],
        Text(title,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 25,
                height: 1.15,
                fontWeight: FontWeight.w800,
                fontVariations: [FontVariation('wght', 800)],
                color: PeerTokens.navy)),
        const SizedBox(height: 8),
        Text(body,
            textAlign: TextAlign.center,
            style: const TextStyle(
                fontSize: 15.5,
                height: 1.4,
                fontWeight: FontWeight.w500,
                fontVariations: [FontVariation('wght', 500)],
                color: PeerTokens.grey)),
        if (below != null) ...[const SizedBox(height: 14), below!],
        const SizedBox(height: 8),
      ]);
}

typedef _Tile = (IconData, Color, Color, String);

class _Tiles extends StatelessWidget {
  const _Tiles({required this.tiles});
  final List<_Tile> tiles;
  @override
  Widget build(BuildContext context) => Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          for (final (icon, bg, fg, label) in tiles)
            Expanded(
              child: Column(children: [
                Container(
                  width: 62,
                  height: 62,
                  decoration: BoxDecoration(color: bg, shape: BoxShape.circle),
                  child: Icon(icon, size: 30, color: fg),
                ),
                const SizedBox(height: 8),
                FittedBox(
                  fit: BoxFit.scaleDown,
                  child: Text(label,
                      textAlign: TextAlign.center,
                      style: const TextStyle(
                          fontSize: 14,
                          height: 1.2,
                          fontWeight: FontWeight.w700,
                          fontVariations: [FontVariation('wght', 700)],
                          color: PeerTokens.navy)),
                ),
              ]),
            ),
        ],
      );
}

typedef _Row = (IconData, Color, Color, String, String);

class _Rows extends StatelessWidget {
  const _Rows({required this.rows});
  final List<_Row> rows;
  @override
  Widget build(BuildContext context) => Column(children: [
        for (var i = 0; i < rows.length; i++) ...[
          if (i > 0) const Divider(height: 14, color: PeerTokens.line),
          Row(children: [
            Container(
              width: 50,
              height: 50,
              decoration: BoxDecoration(
                  color: rows[i].$2, borderRadius: BorderRadius.circular(14)),
              child: Icon(rows[i].$1, size: 26, color: rows[i].$3),
            ),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(rows[i].$4,
                        style: const TextStyle(
                            fontSize: 16,
                            fontWeight: FontWeight.w800,
                            fontVariations: [FontVariation('wght', 800)],
                            color: PeerTokens.navy)),
                    const SizedBox(height: 2),
                    Text(rows[i].$5,
                        style: const TextStyle(
                            fontSize: 13.5,
                            height: 1.3,
                            color: PeerTokens.grey)),
                  ]),
            ),
          ]),
        ],
      ]);
}

class _Dots extends StatelessWidget {
  const _Dots({required this.index, required this.count});
  final int index, count;
  @override
  Widget build(BuildContext context) => Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          for (var i = 0; i < count; i++)
            AnimatedContainer(
              duration: const Duration(milliseconds: 200),
              width: i == index ? 26 : 10,
              height: 10,
              margin: const EdgeInsets.symmetric(horizontal: 4),
              decoration: BoxDecoration(
                  color: i == index ? PeerTokens.green : _dotIdle,
                  borderRadius: BorderRadius.circular(5)),
            ),
        ],
      );
}
