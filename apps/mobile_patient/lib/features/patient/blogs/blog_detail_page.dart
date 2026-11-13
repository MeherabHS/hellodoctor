import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';
import 'article_content.dart';
import 'article_quiz_section.dart';
import '../education/education_topics.dart';
import '../education/education_widgets.dart';

/// Existing article detail flow: Doctor plain text/references are preserved;
/// Admin content uses safe Markdown, localized metadata and authenticated covers.
class BlogDetailPage extends StatefulWidget {
  const BlogDetailPage(
      {super.key, required this.articleId, required this.repo});
  final String articleId;
  final BlogsRepository repo;

  @override
  State<BlogDetailPage> createState() => _BlogDetailPageState();
}

class _BlogDetailPageState extends State<BlogDetailPage> {
  late Future<BlogArticleDto> _future;

  @override
  void initState() {
    super.initState();
    _future = widget.repo.detail(widget.articleId);
  }

  /// Reader per the product owner's article designs (flows, right panel):
  /// the article title in the header, cover, large title, topic chip, read
  /// time and date, then the body with green section headings and tick
  /// bullets. Bookmark and Share are not shown: there is no bookmark store
  /// and articles need sign-in, so neither could work honestly yet.
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    return FutureBuilder<BlogArticleDto>(
      future: _future,
      builder: (context, snapshot) {
        final article = snapshot.data;
        return Scaffold(
          backgroundColor: PatientTokens.bg,
          body: SafeArea(
            child: Column(children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(6, 6, 16, 4),
                child: Row(children: [
                  IconButton(
                    onPressed: () => Navigator.of(context).maybePop(),
                    tooltip:
                        MaterialLocalizations.of(context).backButtonTooltip,
                    icon: const Icon(Icons.arrow_back_rounded,
                        size: 24, color: EduColors.heading),
                  ),
                  const SizedBox(width: 4),
                  Expanded(
                      child: Text(
                          article?.titleFor(language) ?? l10n.blogsTitle,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                              fontSize: 17,
                              height: 1.25,
                              fontWeight: FontWeight.w800,
                              color: EduColors.heading))),
                ]),
              ),
              Expanded(child: _body(context, snapshot, l10n, language)),
            ]),
          ),
        );
      },
    );
  }

  Widget _body(BuildContext context, AsyncSnapshot<BlogArticleDto> snapshot,
      AppLocalizations l10n, String language) {
    if (snapshot.connectionState != ConnectionState.done) {
      return ListView(
        padding: const EdgeInsets.all(18),
        children: const [SkeletonBlock(height: 200)],
      );
    }
    if (snapshot.hasError) {
      return ErrorPanel(l10n.blogsDetailLoadError,
          onRetry: () =>
              setState(() => _future = widget.repo.detail(widget.articleId)));
    }
    final article = snapshot.data!;
    final topic = EducationTopic.fromApiKey(article.topicKey);
    final chip = topic?.title(l10n) ?? article.categoryFor(language);
    return ListView(
      padding: const EdgeInsets.fromLTRB(18, 4, 18, 32),
      children: [
        if (article.hasCover)
          ArticleCover(
              articleId: article.id,
              mediaId: article.coverImageId!,
              repo: widget.repo),
        Text(article.titleFor(language),
            style: const TextStyle(
                fontSize: 26,
                height: 1.15,
                fontWeight: FontWeight.w800,
                color: EduColors.heading)),
        if (chip.isNotEmpty) ...[
          const SizedBox(height: 10),
          Align(
            alignment: Alignment.centerLeft,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
              decoration: BoxDecoration(
                  color: EduColors.chipBg,
                  borderRadius: BorderRadius.circular(20)),
              child: Text(chip,
                  style: const TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w700,
                      color: EduColors.sectionGreen)),
            ),
          ),
        ],
        const SizedBox(height: 12),
        if (article.standaloneQuiz && article.quiz != null)
          Row(children: [
            const Icon(Icons.quiz_outlined, size: 17, color: EduColors.meta),
            const SizedBox(width: 5),
            Text(l10n.eduQuizQuestions(article.quiz!.questionCount),
                style: const TextStyle(fontSize: 14, color: EduColors.meta)),
          ])
        else
          EduMetaRow(article: article, size: 14),
        if (!article.isAdmin) ...[
          const SizedBox(height: 10),
          Row(children: [
            CircleAvatar(
              radius: 14,
              backgroundColor: PatientTokens.primaryLight,
              child: Text(_initialsOf(article.authorName),
                  style: const TextStyle(
                      fontSize: 10,
                      fontWeight: FontWeight.w800,
                      color: PatientTokens.primaryDark)),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text('${article.authorName} • ${article.specialty}',
                  style: const TextStyle(
                      fontSize: 12, color: PatientTokens.textSub)),
            ),
          ]),
        ],
        const Divider(height: 28, color: EduColors.divider),
        if (!article.isAdmin)
          Text(article.summaryFor(language),
              style: const TextStyle(
                  fontSize: 13,
                  fontStyle: FontStyle.italic,
                  color: PatientTokens.textSub,
                  height: 1.5)),
        if (article.contentFormat == 'markdown')
          ArticleMarkdown(body: article.bodyFor(language))
        else
          Text(article.bodyFor(language),
              style: const TextStyle(
                  fontSize: 14.5, color: EduColors.heading, height: 1.45)),
        if (article.referencesList.isNotEmpty) ...[
          const SizedBox(height: 20),
          Text(l10n.blogsReferences,
              style: const TextStyle(
                  fontWeight: FontWeight.w800, color: PatientTokens.text)),
          const SizedBox(height: 8),
          for (final ref in article.referencesList)
            Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Text('• $ref',
                  style: const TextStyle(
                      fontSize: 11.5, color: PatientTokens.textSub)),
            ),
        ],
        if (article.isAdmin && article.quiz != null) ...[
          const SizedBox(height: 24),
          ArticleQuizSection(
              key: ValueKey(article.quiz!.revisionId),
              articleId: article.id,
              publication: article.quiz!,
              repository: widget.repo.quizzes,
              standalone: article.standaloneQuiz,
              onRefreshArticle: () => setState(
                  () => _future = widget.repo.detail(widget.articleId))),
        ],
      ],
    );
  }

  String _initialsOf(String name) {
    final words = name
        .replaceAll('Dr.', '')
        .trim()
        .split(RegExp(r'\s+'))
        .where((w) => w.isNotEmpty)
        .toList();
    if (words.isEmpty) return '?';
    if (words.length == 1) return words.first.substring(0, 1).toUpperCase();
    return (words.first.substring(0, 1) + words.last.substring(0, 1))
        .toUpperCase();
  }
}
