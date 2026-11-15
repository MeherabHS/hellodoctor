import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../core/l10n/kenya_time.dart';
import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import '../blogs/blogs_models.dart';
import '../blogs/blogs_repository.dart';
import '../modules/afya_module_widgets.dart';
import 'education_topics.dart';

/// Colours sampled from the product owner's education flows
/// (`docs/design/education/flow-*.png`).
class EduColors {
  static const heading = Color(0xFF0B2A3F); // "Articles (5)", titles
  static const sectionGreen = Color(0xFF0F7B2B); // reader section headings
  static const checkGreen = Color(0xFF1E9638); // reader list ticks
  static const chipBg = Color(0xFFD7F6E4);
  static const meta = Color(0xFF33485A);
  static const divider = Color(0xFFE6ECEF);
}

/// "Nov 10, 2026" in Kenya time, localized.
String eduDate(BuildContext context, DateTime instant) =>
    DateFormat.yMMMd(Localizations.localeOf(context).toLanguageTag())
        .format(kenyaTime(instant.toUtc()));

/// Topic banner: native title and description over the topic's colour, with
/// the topic artwork (cropped from the reference) on the right. The artwork's
/// left edge fades into the card so no seam shows on any width.
class TopicBanner extends StatelessWidget {
  const TopicBanner({super.key, required this.topic});
  final EducationTopic topic;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return ClipRRect(
      borderRadius: BorderRadius.circular(14),
      child: Container(
        decoration: BoxDecoration(
            gradient: LinearGradient(
                colors: [topic.bannerColor, topic.bannerArtEdge])),
        child: Stack(children: [
          Positioned(
            right: 0,
            top: 0,
            bottom: 0,
            width: topic.bannerArtWidth,
            child: ShaderMask(
              blendMode: BlendMode.dstIn,
              shaderCallback: (r) => const LinearGradient(
                  colors: [Colors.transparent, Colors.black],
                  stops: [0, .16]).createShader(r),
              child: Image.asset(topic.bannerAsset,
                  fit: BoxFit.cover,
                  alignment: Alignment.centerRight,
                  filterQuality: FilterQuality.medium,
                  excludeFromSemantics: true),
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(18, 22, topic.bannerArtWidth - 6, 22),
            child: ConstrainedBox(
              constraints: const BoxConstraints(minHeight: 82),
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(topic.title(l),
                        style: const TextStyle(
                            fontSize: 22,
                            height: 1.15,
                            fontWeight: FontWeight.w800,
                            color: EduColors.heading)),
                    const SizedBox(height: 6),
                    Text(topic.bannerDescription(l),
                        style: const TextStyle(
                            fontSize: 14,
                            height: 1.35,
                            color: EduColors.heading)),
                  ]),
            ),
          ),
        ]),
      ),
    );
  }
}

/// Empty state: illustration on top, title and message below, in a tinted
/// rounded card. Used per topic (its own art) and per tab (Videos, Quizzes,
/// Audio).
class EduEmptyCard extends StatelessWidget {
  const EduEmptyCard(
      {super.key,
      required this.asset,
      required this.color,
      required this.title,
      required this.body,
      this.artHeight});
  final String asset;
  final Color color;
  final String title, body;

  /// Fixed height for small interim artwork; null = full card width.
  final double? artHeight;
  @override
  Widget build(BuildContext context) => Container(
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(14),
        ),
        clipBehavior: Clip.antiAlias,
        padding: const EdgeInsets.only(bottom: 26),
        child: Column(children: [
          if (artHeight == null)
            // The crop's bottom rows are a slightly different tint from the
            // card below; fade them so no seam shows.
            ShaderMask(
              blendMode: BlendMode.dstIn,
              shaderCallback: (r) => const LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [Colors.black, Colors.transparent],
                  stops: [.86, 1]).createShader(r),
              child: Image.asset(asset,
                  width: double.infinity,
                  fit: BoxFit.fitWidth,
                  filterQuality: FilterQuality.medium,
                  excludeFromSemantics: true),
            )
          else
            Padding(
              padding: const EdgeInsets.only(top: 20),
              child: Image.asset(asset,
                  height: artHeight,
                  fit: BoxFit.contain,
                  filterQuality: FilterQuality.medium,
                  excludeFromSemantics: true),
            ),
          const SizedBox(height: 8),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: Column(children: [
              Text(title,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.w800,
                      color: EduColors.heading)),
              const SizedBox(height: 8),
              Text(body,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                      fontSize: 15, height: 1.4, color: EduColors.heading)),
            ]),
          ),
        ]),
      );
}

/// Square thumbnail: the article's authenticated cover (memory only, never a
/// disk cache), else the topic artwork.
class EduThumb extends StatefulWidget {
  const EduThumb(
      {super.key,
      required this.article,
      required this.repo,
      required this.fallback,
      this.size = 66});
  final BlogArticleDto article;
  final BlogsRepository repo;
  final String fallback;
  final double size;
  @override
  State<EduThumb> createState() => _EduThumbState();
}

class _EduThumbState extends State<EduThumb> {
  Future<List<int>>? _bytes;
  @override
  void initState() {
    super.initState();
    if (widget.article.hasCover) {
      _bytes = widget.repo.cover(widget.article.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final fallback =
        AfyaArt(widget.fallback, width: widget.size, height: widget.size);
    if (_bytes == null) return fallback;
    return FutureBuilder<List<int>>(
        future: _bytes,
        builder: (context, s) => s.hasData
            ? ClipRRect(
                borderRadius: BorderRadius.circular(12),
                child: Image.memory(Uint8List.fromList(s.data!),
                    width: widget.size,
                    height: widget.size,
                    fit: BoxFit.cover,
                    cacheWidth: 240,
                    errorBuilder: (_, __, ___) => fallback))
            : fallback);
  }
}

/// Clock "5 min read" · calendar "Nov 10, 2026".
class EduMetaRow extends StatelessWidget {
  const EduMetaRow({super.key, required this.article, this.size = 12.5});
  final BlogArticleDto article;
  final double size;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    final style = TextStyle(fontSize: size, color: EduColors.meta);
    return Wrap(
        spacing: 14,
        runSpacing: 4,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Row(mainAxisSize: MainAxisSize.min, children: [
            Icon(Icons.schedule_rounded, size: size + 3, color: EduColors.meta),
            const SizedBox(width: 5),
            Text(l.eduMinRead(article.readMinutesFor(language)), style: style),
          ]),
          if (article.publishedAt != null)
            Row(mainAxisSize: MainAxisSize.min, children: [
              Icon(Icons.calendar_today_outlined,
                  size: size + 1, color: EduColors.meta),
              const SizedBox(width: 5),
              Text(eduDate(context, article.publishedAt!), style: style),
            ]),
        ]);
  }
}

/// One article row of a topic list (flows, middle panel).
class EduArticleRow extends StatelessWidget {
  const EduArticleRow(
      {super.key,
      required this.article,
      required this.repo,
      required this.fallbackArt,
      required this.onTap,
      this.trailingMeta});
  final BlogArticleDto article;
  final BlogsRepository repo;
  final String fallbackArt;
  final VoidCallback onTap;

  /// Replaces the read-time row (Quizzes tab: question count).
  final Widget? trailingMeta;
  @override
  Widget build(BuildContext context) {
    final language = Localizations.localeOf(context).languageCode;
    return InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 10),
        decoration: const BoxDecoration(
            border: Border(bottom: BorderSide(color: EduColors.divider))),
        child: Row(children: [
          EduThumb(article: article, repo: repo, fallback: fallbackArt),
          const SizedBox(width: 14),
          Expanded(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(article.titleFor(language),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 15,
                      height: 1.25,
                      fontWeight: FontWeight.w800,
                      color: EduColors.heading)),
              const SizedBox(height: 7),
              trailingMeta ?? EduMetaRow(article: article),
            ]),
          ),
          const Icon(Icons.chevron_right_rounded,
              size: 26, color: EduColors.heading),
        ]),
      ),
    );
  }
}

/// A video row: thumbnail with a play badge; tapping opens YouTube outside the
/// app. Only the backend's canonical YouTube URL is ever opened.
class EduVideoRow extends StatelessWidget {
  const EduVideoRow(
      {super.key,
      required this.video,
      required this.repo,
      required this.fallbackArt});
  final BlogArticleDto video;
  final BlogsRepository repo;
  final String fallbackArt;

  Future<void> _open(BuildContext context) async {
    final l = AppLocalizations.of(context);
    final messenger = ScaffoldMessenger.maybeOf(context);
    var opened = false;
    try {
      opened = await launchUrl(video.videoUrl!,
          mode: LaunchMode.externalApplication);
    } catch (_) {}
    if (!opened) {
      messenger?.showSnackBar(SnackBar(content: Text(l.eduVideoOpenFailed)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    return Semantics(
      button: true,
      label: '${video.titleFor(language)}. ${l.eduWatchOnYouTube}',
      excludeSemantics: true,
      child: InkWell(
        onTap: () => _open(context),
        child: Container(
          padding: const EdgeInsets.symmetric(vertical: 10),
          decoration: const BoxDecoration(
              border: Border(bottom: BorderSide(color: EduColors.divider))),
          child: Row(children: [
            Stack(alignment: Alignment.center, children: [
              EduThumb(article: video, repo: repo, fallback: fallbackArt),
              Container(
                width: 30,
                height: 30,
                decoration: const BoxDecoration(
                    color: Color(0xCC000000), shape: BoxShape.circle),
                child: const Icon(Icons.play_arrow_rounded,
                    color: Colors.white, size: 22),
              ),
            ]),
            const SizedBox(width: 14),
            Expanded(
              child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(video.titleFor(language),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                            fontSize: 15,
                            height: 1.25,
                            fontWeight: FontWeight.w800,
                            color: EduColors.heading)),
                    const SizedBox(height: 7),
                    Row(children: [
                      const Icon(Icons.smart_display_outlined,
                          size: 16, color: Color(0xFFD32F2F)),
                      const SizedBox(width: 5),
                      Flexible(
                        child: Text(l.eduWatchOnYouTube,
                            style: const TextStyle(
                                fontSize: 12.5, color: EduColors.meta)),
                      ),
                    ]),
                  ]),
            ),
            const Icon(Icons.open_in_new_rounded,
                size: 20, color: EduColors.heading),
          ]),
        ),
      ),
    );
  }
}

/// Loads a server-filtered feed page by page.
class EduFeedController extends ChangeNotifier {
  EduFeedController(this.repo,
      {this.topic, this.videos = false, this.standaloneQuizzes = false});
  final BlogsRepository repo;
  final String? topic;
  final bool videos;

  /// Standalone quizzes (Admin Quiz Manager) instead of articles.
  final bool standaloneQuizzes;

  final items = <BlogArticleDto>[];
  BlogCursor? _next;
  bool loading = false, failed = false, loaded = false;
  bool get hasMore => _next != null;

  Future<void> load({required String language, bool more = false}) async {
    if (loading) return;
    loading = true;
    failed = false;
    if (!more) {
      items.clear();
      _next = null;
    }
    notifyListeners();
    try {
      final page = await repo.discover(
          language: language,
          cursor: more ? _next : null,
          limit: 50,
          topic: topic,
          videos: videos,
          standaloneQuizzes: standaloneQuizzes);
      items.addAll(page.items);
      _next = page.next;
      loaded = true;
    } catch (_) {
      failed = true;
    } finally {
      loading = false;
      notifyListeners();
    }
  }
}

class EduErrorPanel extends StatelessWidget {
  const EduErrorPanel({super.key, required this.onRetry});
  final VoidCallback onRetry;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 24),
      child: Column(children: [
        const Icon(Icons.wifi_off_rounded, color: PatientTokens.textSub),
        const SizedBox(height: 8),
        Text(l.eduLoadError,
            textAlign: TextAlign.center,
            style: const TextStyle(color: PatientTokens.textSub)),
        const SizedBox(height: 8),
        OutlinedButton(onPressed: onRetry, child: Text(l.eduRetry)),
      ]),
    );
  }
}
