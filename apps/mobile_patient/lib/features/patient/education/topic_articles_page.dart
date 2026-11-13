import 'package:flutter/material.dart';

import '../../../l10n/app_localizations.dart';
import '../blogs/blog_detail_page.dart';
import '../blogs/blogs_repository.dart';
import '../home/brand_header.dart';
import '../modules/afya_module_widgets.dart';
import '../modules/patient_modules.dart';
import 'education_topics.dart';
import 'education_widgets.dart';

/// A topic's page (product owner's flows, middle panel; empty variant per
/// `docs/design/education/topic-empty-*.jpg`): the topic banner, "Articles (N)"
/// and the topic's published articles from the server, or the topic's own
/// empty state. Articles come only from the backend (`?topic=`); nothing here
/// is sample content.
class TopicArticlesPage extends StatefulWidget {
  const TopicArticlesPage(
      {super.key,
      required this.topic,
      required this.repo,
      required this.nav,
      this.appHeader});
  final EducationTopic topic;
  final BlogsRepository repo;
  final PatientShellNav nav;
  final AppHeaderDeps? appHeader;
  @override
  State<TopicArticlesPage> createState() => _TopicArticlesPageState();
}

class _TopicArticlesPageState extends State<TopicArticlesPage> {
  late final EduFeedController _feed =
      EduFeedController(widget.repo, topic: widget.topic.apiKey);
  String? _language;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final language = Localizations.localeOf(context).languageCode;
    if (language != _language) {
      _language = language;
      _feed.load(language: language);
    }
  }

  @override
  void dispose() {
    _feed.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final topic = widget.topic;
    return AfyaModuleScaffold(
      title: topic.title(l),
      nav: widget.nav,
      appHeader: widget.appHeader,
      child: RefreshIndicator(
        onRefresh: () => _feed.load(language: _language!),
        child: ListenableBuilder(
          listenable: _feed,
          builder: (context, _) => ListView(
            physics: const AlwaysScrollableScrollPhysics(),
            padding: const EdgeInsets.fromLTRB(16, 6, 16, 24),
            children: [
              TopicBanner(topic: topic),
              const SizedBox(height: 22),
              if (_feed.loaded || _feed.items.isNotEmpty)
                Text(l.eduArticlesCount(_feed.items.length),
                    style: const TextStyle(
                        fontSize: 22,
                        fontWeight: FontWeight.w800,
                        color: EduColors.heading)),
              const SizedBox(height: 12),
              if (_feed.loading && _feed.items.isEmpty)
                const Padding(
                    padding: EdgeInsets.all(32),
                    child: Center(child: CircularProgressIndicator()))
              else if (_feed.failed && _feed.items.isEmpty)
                EduErrorPanel(onRetry: () => _feed.load(language: _language!))
              else if (_feed.items.isEmpty)
                EduEmptyCard(
                    asset: topic.emptyAsset,
                    color: topic.emptyColor,
                    title: l.eduNoArticlesTitle,
                    body: topic.emptyBody(l))
              else ...[
                const Divider(height: 1, color: EduColors.divider),
                for (final a in _feed.items)
                  EduArticleRow(
                      article: a,
                      repo: widget.repo,
                      fallbackArt: topic.thumbnail,
                      onTap: () => Navigator.of(context).push(MaterialPageRoute(
                          builder: (_) => BlogDetailPage(
                              articleId: a.id, repo: widget.repo)))),
                if (_feed.hasMore)
                  Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Center(
                      child: OutlinedButton(
                          onPressed: _feed.loading
                              ? null
                              : () =>
                                  _feed.load(language: _language!, more: true),
                          child: Text(l.eduLoadMore)),
                    ),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
