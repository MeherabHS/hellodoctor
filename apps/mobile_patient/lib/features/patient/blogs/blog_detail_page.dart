import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';
import 'article_content.dart';
import 'article_quiz_section.dart';

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

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 8, 8, 0),
            child: Row(children: [
              IconButton(
                onPressed: () => Navigator.of(context).maybePop(),
                icon: const Icon(Icons.arrow_back, size: 20),
              ),
              Expanded(
                  child: Text(l10n.blogsTitle,
                      style: const TextStyle(
                          fontWeight: FontWeight.w800,
                          color: PatientTokens.text))),
            ]),
          ),
          Expanded(
            child: FutureBuilder<BlogArticleDto>(
              future: _future,
              builder: (context, snapshot) {
                if (snapshot.connectionState != ConnectionState.done) {
                  return ListView(
                    padding: const EdgeInsets.all(18),
                    children: const [SkeletonBlock(height: 200)],
                  );
                }
                if (snapshot.hasError) {
                  return ErrorPanel(l10n.blogsDetailLoadError,
                      onRetry: () => setState(() =>
                          _future = widget.repo.detail(widget.articleId)));
                }
                final article = snapshot.data!;
                return ListView(
                  padding: const EdgeInsets.fromLTRB(18, 8, 18, 32),
                  children: [
                    if (article.hasCover)
                      ArticleCover(
                          articleId: article.id,
                          mediaId: article.coverImageId!,
                          repo: widget.repo),
                    if (article.categoryFor(language).isNotEmpty)
                      Text(article.categoryFor(language),
                          style: const TextStyle(
                              fontSize: 11, color: PatientTokens.primary)),
                    Text(article.titleFor(language),
                        style: const TextStyle(
                            fontSize: 19,
                            fontWeight: FontWeight.w800,
                            color: PatientTokens.text)),
                    const SizedBox(height: 8),
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
                        child: Text(
                            article.isAdmin
                                ? l10n.blogsEditorial
                                : '${article.authorName} • ${article.specialty}',
                            style: const TextStyle(
                                fontSize: 12, color: PatientTokens.textSub)),
                      ),
                      if (article.publishedAt != null)
                        Text(_dateLabel(article.publishedAt!),
                            style: const TextStyle(
                                fontSize: 11, color: PatientTokens.textSub)),
                    ]),
                    const Divider(height: 28),
                    if (!article.isAdmin)
                      Text(article.summaryFor(language),
                          style: const TextStyle(
                              fontSize: 13,
                              fontStyle: FontStyle.italic,
                              color: PatientTokens.textSub,
                              height: 1.5)),
                    const SizedBox(height: 14),
                    if (article.contentFormat == 'markdown')
                      ArticleMarkdown(body: article.bodyFor(language))
                    else
                      Text(article.bodyFor(language),
                          style: const TextStyle(
                              fontSize: 13.5,
                              color: PatientTokens.text,
                              height: 1.6)),
                    if (article.referencesList.isNotEmpty) ...[
                      const SizedBox(height: 20),
                      Text(l10n.blogsReferences,
                          style: const TextStyle(
                              fontWeight: FontWeight.w800,
                              color: PatientTokens.text)),
                      const SizedBox(height: 8),
                      for (final ref in article.referencesList)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 6),
                          child: Text('• $ref',
                              style: const TextStyle(
                                  fontSize: 11.5,
                                  color: PatientTokens.textSub)),
                        ),
                    ],
                    if (article.isAdmin && article.quiz != null) ...[
                      const SizedBox(height: 24),
                      ArticleQuizSection(
                          key: ValueKey(article.quiz!.revisionId),
                          articleId: article.id,
                          publication: article.quiz!,
                          repository: widget.repo.quizzes,
                          onRefreshArticle: () => setState(() =>
                              _future = widget.repo.detail(widget.articleId))),
                    ],
                  ],
                );
              },
            ),
          ),
        ]),
      ),
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

  String _dateLabel(DateTime d) => '${d.day}/${d.month}/${d.year}';
}
