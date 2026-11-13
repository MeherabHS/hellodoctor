import 'package:flutter/material.dart';

import '../../../l10n/app_localizations.dart';
import '../blogs/blog_detail_page.dart';
import '../blogs/blogs_repository.dart';
import 'education_topics.dart';
import 'education_widgets.dart';

/// Hub tab content for Videos (published YouTube entries) and Quizzes, each
/// with its empty state. Quizzes lists standalone quizzes (Admin Quiz Manager)
/// first, then published articles that include a quiz. All from the server.
enum EduTabKind { videos, quizzes }

class EduTabFeed extends StatefulWidget {
  const EduTabFeed({super.key, required this.kind, required this.repo});
  final EduTabKind kind;
  final BlogsRepository repo;
  @override
  State<EduTabFeed> createState() => _EduTabFeedState();
}

class _EduTabFeedState extends State<EduTabFeed> {
  late final EduFeedController _feed =
      EduFeedController(widget.repo, videos: widget.kind == EduTabKind.videos);

  /// Quizzes tab only: quizzes that are not attached to an article.
  late final EduFeedController? _standalone = widget.kind == EduTabKind.quizzes
      ? EduFeedController(widget.repo, standaloneQuizzes: true)
      : null;
  String? _language;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final language = Localizations.localeOf(context).languageCode;
    if (language != _language) {
      _language = language;
      _feed.load(language: language);
      _standalone?.load(language: language);
    }
  }

  @override
  void dispose() {
    _feed.dispose();
    _standalone?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final videos = widget.kind == EduTabKind.videos;
    final standalone = _standalone;
    return ListenableBuilder(
      listenable: Listenable.merge([_feed, if (standalone != null) standalone]),
      builder: (context, _) {
        final anyLoading = _feed.loading || (standalone?.loading ?? false);
        final noneYet =
            _feed.items.isEmpty && (standalone?.items.isEmpty ?? true);
        if (anyLoading && noneYet) {
          return const Padding(
              padding: EdgeInsets.all(32),
              child: Center(child: CircularProgressIndicator()));
        }
        if (_feed.failed && _feed.items.isEmpty) {
          return EduErrorPanel(onRetry: () => _feed.load(language: _language!));
        }
        final items = videos
            ? _feed.items.where((a) => a.isVideo).toList()
            : [
                ...?standalone?.items.where((a) => a.quiz != null),
                ..._feed.items.where((a) => a.quiz != null),
              ];
        if (items.isEmpty && !_feed.hasMore) {
          return videos
              ? EduEmptyCard(
                  asset: 'assets/education/tab_empty_videos.png',
                  color: const Color(0xFFEAF6FE),
                  title: l.eduNoVideosTitle,
                  body: l.eduNoVideosBody)
              : EduEmptyCard(
                  asset: 'assets/education/tab_empty_quizzes.png',
                  color: const Color(0xFFF1EDFE),
                  title: l.eduNoQuizzesTitle,
                  body: l.eduNoQuizzesBody);
        }
        return Column(children: [
          const Divider(height: 1, color: EduColors.divider),
          for (final a in items)
            if (videos)
              EduVideoRow(
                  video: a, repo: widget.repo, fallbackArt: _art(a.topicKey))
            else
              EduArticleRow(
                  article: a,
                  repo: widget.repo,
                  fallbackArt: _art(a.topicKey),
                  trailingMeta: Row(children: [
                    const Icon(Icons.quiz_outlined,
                        size: 16, color: EduColors.meta),
                    const SizedBox(width: 5),
                    Text(l.eduQuizQuestions(a.quiz!.questionCount),
                        style: const TextStyle(
                            fontSize: 12.5, color: EduColors.meta)),
                  ]),
                  onTap: () => Navigator.of(context).push(MaterialPageRoute(
                      builder: (_) =>
                          BlogDetailPage(articleId: a.id, repo: widget.repo)))),
          if (_feed.hasMore)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: OutlinedButton(
                  onPressed: _feed.loading
                      ? null
                      : () => _feed.load(language: _language!, more: true),
                  child: Text(l.eduLoadMore)),
            ),
        ]);
      },
    );
  }

  String _art(String? topicKey) =>
      EducationTopic.fromApiKey(topicKey)?.thumbnail ?? 'topic_hiv_basics';
}

/// Audio has no content model yet; this is its honest empty state.
class EduAudioEmpty extends StatelessWidget {
  const EduAudioEmpty({super.key});
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return EduEmptyCard(
        asset: 'assets/education/tab_empty_audio.png',
        color: const Color(0xFFFEECEF),
        title: l.eduNoAudioTitle,
        body: l.eduNoAudioBody);
  }
}
