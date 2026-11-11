import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'blog_detail_page.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';
import 'article_content.dart';
import 'blog_discovery_controller.dart';

/// Existing Blogs tab: bounded server search and stable cursor pagination.
class BlogsPage extends StatefulWidget {
  const BlogsPage({super.key, required this.repo});
  final BlogsRepository repo;
  @override
  State<BlogsPage> createState() => _BlogsPageState();
}

class _BlogsPageState extends State<BlogsPage> {
  late final BlogDiscoveryController controller;
  String? _language;
  @override
  void initState() {
    super.initState();
    controller = BlogDiscoveryController(widget.repo);
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final language = Localizations.localeOf(context).languageCode;
    if (_language != language) {
      _language = language;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _language == language) controller.setLanguage(language);
      });
    }
  }

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Scaffold(
        backgroundColor: PatientTokens.bg,
        body: SafeArea(
            child: Column(children: [
          PatientPageHeader(l10n.blogsHeading, l10n.blogsSubtitle),
          Padding(
              padding: const EdgeInsets.fromLTRB(18, 10, 18, 0),
              child: TextField(
                maxLength: 160,
                onChanged: controller.search,
                decoration: InputDecoration(
                    counterText: '',
                    prefixIcon: const Icon(Icons.search),
                    hintText: l10n.blogsSearchHint),
              )),
          Expanded(
              child: AnimatedBuilder(
                  animation: controller,
                  builder: (context, _) {
                    final c = controller;
                    Widget content;
                    if (c.loading) {
                      content = ListView(
                          physics: const AlwaysScrollableScrollPhysics(),
                          padding: const EdgeInsets.all(18),
                          children: const [
                            SkeletonBlock(height: 90),
                            SizedBox(height: 10),
                            SkeletonBlock(height: 90)
                          ]);
                    } else if (c.failed) {
                      content = ListView(
                          physics: const AlwaysScrollableScrollPhysics(),
                          children: [
                            ErrorPanel(l10n.blogsLoadError, onRetry: c.refresh)
                          ]);
                    } else if (c.items.isEmpty) {
                      content = ListView(
                          physics: const AlwaysScrollableScrollPhysics(),
                          children: [
                            EmptyState(
                                icon: Icons.article_outlined,
                                title: c.query.trim().isEmpty
                                    ? l10n.blogsEmptyTitle
                                    : l10n.blogsSearchEmptyTitle,
                                message: c.query.trim().isEmpty
                                    ? l10n.blogsEmptyMessage
                                    : l10n.blogsSearchEmptyMessage)
                          ]);
                    } else {
                      content = ListView.builder(
                          physics: const AlwaysScrollableScrollPhysics(),
                          padding: const EdgeInsets.fromLTRB(18, 10, 18, 24),
                          itemCount: c.items.length + 1,
                          itemBuilder: (context, index) {
                            if (index == c.items.length) {
                              return Column(children: [
                                if (c.pageFailed)
                                  InlineError(l10n.blogsPageError,
                                      onRetry: c.loadMore),
                                if (c.next != null)
                                  TextButton(
                                      key: const ValueKey('blogs-load-more'),
                                      onPressed:
                                          c.loadingMore ? null : c.loadMore,
                                      child: Text(c.loadingMore
                                          ? l10n.blogsLoadingMore
                                          : l10n.blogsLoadMore))
                                else
                                  Text(l10n.blogsEnd),
                              ]);
                            }
                            final article = c.items[index];
                            return Padding(
                                key: ValueKey('blog-${article.id}'),
                                padding: const EdgeInsets.only(bottom: 10),
                                child: _ArticleTile(
                                    article: article,
                                    repo: widget.repo,
                                    onTap: () => Navigator.of(context).push(
                                        MaterialPageRoute(
                                            builder: (_) => BlogDetailPage(
                                                articleId: article.id,
                                                repo: widget.repo)))));
                          });
                    }
                    return RefreshIndicator(
                        onRefresh: c.refresh, child: content);
                  })),
        ])));
  }
}

class _ArticleTile extends StatelessWidget {
  const _ArticleTile(
      {required this.article, required this.onTap, required this.repo});
  final BlogArticleDto article;
  final BlogsRepository repo;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final language = Localizations.localeOf(context).languageCode;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: PatientCard(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          if (article.hasCover)
            ArticleCover(
                articleId: article.id,
                mediaId: article.coverImageId!,
                repo: repo),
          Text(article.titleFor(language),
              style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w700,
                  color: PatientTokens.text)),
          const SizedBox(height: 4),
          Text(article.summaryFor(language),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(
                  fontSize: 11, color: PatientTokens.textSub, height: 1.4)),
          const SizedBox(height: 8),
          Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
            Expanded(
              child: Text(
                  article.isAdmin
                      ? l10n.blogsEditorial
                      : l10n.blogsByAuthor(
                          article.authorName, article.specialty),
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
