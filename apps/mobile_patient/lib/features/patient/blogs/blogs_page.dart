import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'blog_detail_page.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';

/// `prototype/index.html` view-13 — "Health Blogs & Medical Library", made
/// real against `GET /articles`. The prototype's "Featured Editorial" hero
/// and per-article "5 min read"/category badges are static mockup text with
/// no backing field on `DoctorArticle` — not reproduced here (see
/// `blogs_models.dart`). Search is client-side substring filtering over the
/// loaded page, since the backend's `ArticleQuery` has no search parameter.
class BlogsPage extends StatefulWidget {
  const BlogsPage({super.key, required this.repo});
  final BlogsRepository repo;

  @override
  State<BlogsPage> createState() => _BlogsPageState();
}

class _BlogsPageState extends State<BlogsPage> {
  late Future<List<BlogArticleDto>> _future;
  String _search = '';

  @override
  void initState() {
    super.initState();
    _future = widget.repo.feed();
  }

  Future<void> _refresh() async {
    final next = widget.repo.feed();
    setState(() => _future = next);
    await next;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
      backgroundColor: PatientTokens.bg,
      body: SafeArea(
        child: Column(children: [
          const PatientPageHeader('Health articles',
              'Practical guidance from verified HelloDoctor physicians'),
          Padding(
            padding: const EdgeInsets.fromLTRB(18, 10, 18, 0),
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 4),
              decoration: BoxDecoration(
                color: PatientTokens.surfaceAlt,
                borderRadius: BorderRadius.circular(PatientTokens.chipRadius),
                border: Border.all(color: PatientTokens.border),
              ),
              child: Row(children: [
                const Icon(Icons.search,
                    color: PatientTokens.textSub, size: 18),
                const SizedBox(width: 8),
                Expanded(
                  child: TextField(
                    onChanged: (v) => setState(() => _search = v),
                    decoration: InputDecoration(
                      border: InputBorder.none,
                      isDense: true,
                      hintText: l10n.blogsSearchHint,
                    ),
                  ),
                ),
              ]),
            ),
          ),
          Expanded(
            child: RefreshIndicator(
              onRefresh: _refresh,
              child: FutureBuilder<List<BlogArticleDto>>(
                future: _future,
                builder: (context, snapshot) {
                  if (snapshot.connectionState != ConnectionState.done) {
                    return ListView(
                      padding: const EdgeInsets.all(18),
                      children: const [
                        SkeletonBlock(height: 90),
                        SizedBox(height: 10),
                        SkeletonBlock(height: 90),
                      ],
                    );
                  }
                  if (snapshot.hasError) {
                    return ErrorPanel(l10n.blogsLoadError, onRetry: _refresh);
                  }
                  final query = _search.trim().toLowerCase();
                  final articles = (snapshot.data ?? const [])
                      .where((a) =>
                          query.isEmpty ||
                          a.title.toLowerCase().contains(query) ||
                          a.summary.toLowerCase().contains(query))
                      .toList();
                  if (articles.isEmpty) {
                    return ListView(children: const [
                      EmptyState(
                          icon: Icons.article_outlined,
                          title: 'No articles yet',
                          message:
                              'Health guides from verified HelloDoctor physicians will appear here.'),
                    ]);
                  }
                  return ListView(
                    padding: const EdgeInsets.fromLTRB(18, 10, 18, 24),
                    children: [
                      for (final article in articles)
                        Padding(
                          padding: const EdgeInsets.only(bottom: 10),
                          child: _ArticleTile(
                            article: article,
                            onTap: () =>
                                Navigator.of(context).push(MaterialPageRoute(
                              builder: (_) => BlogDetailPage(
                                  articleId: article.id, repo: widget.repo),
                            )),
                          ),
                        ),
                    ],
                  );
                },
              ),
            ),
          ),
        ]),
      ),
    );
  }
}

class _ArticleTile extends StatelessWidget {
  const _ArticleTile({required this.article, required this.onTap});
  final BlogArticleDto article;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: PatientCard(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(article.title,
              style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: PatientTokens.text)),
          const SizedBox(height: 4),
          Text(article.summary,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(
                  fontSize: 11, color: PatientTokens.textSub, height: 1.4)),
          const SizedBox(height: 8),
          Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
            Expanded(
              child: Text(
                  l10n.blogsByAuthor(article.authorName, article.specialty),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 11, color: PatientTokens.textSub)),
            ),
            Text(l10n.blogsReadMore,
                style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w700,
                    color: PatientTokens.primary)),
          ]),
        ]),
      ),
    );
  }
}
