import 'package:flutter/material.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'blogs_models.dart';
import 'blogs_repository.dart';

/// `prototype/index.html` view-13's article-detail flow, made real. Shows
/// the full doctor-authored body and reference list — no invented "read
/// time" or category badge (see `blogs_models.dart`'s doc comment).
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
              Text(l10n.blogsTitle,
                  style: const TextStyle(
                      fontWeight: FontWeight.w800, color: PatientTokens.text)),
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
                    Text(article.title,
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
                            '${article.authorName} • ${article.specialty}',
                            style: const TextStyle(
                                fontSize: 12, color: PatientTokens.textSub)),
                      ),
                      if (article.publishedAt != null)
                        Text(_dateLabel(article.publishedAt!),
                            style: const TextStyle(
                                fontSize: 11, color: PatientTokens.textSub)),
                    ]),
                    const Divider(height: 28),
                    Text(article.summary,
                        style: const TextStyle(
                            fontSize: 13,
                            fontStyle: FontStyle.italic,
                            color: PatientTokens.textSub,
                            height: 1.5)),
                    const SizedBox(height: 14),
                    Text(article.body,
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
