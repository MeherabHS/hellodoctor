import 'package:flutter/material.dart';
import '../../core/network/api_client.dart';
import '../../core/widgets/doctor_widgets.dart';
import 'articles_repository.dart';

void openDoctorArticleEditor(BuildContext context, ApiClient api) {
  Navigator.push(
      context,
      MaterialPageRoute(
          builder: (_) => _ArticleEditor(repo: ArticlesRepository(api))));
}

class DoctorArticlesPage extends StatefulWidget {
  const DoctorArticlesPage({super.key, required this.api});
  final ApiClient api;
  @override
  State<DoctorArticlesPage> createState() => _ArticlesState();
}

class _ArticlesState extends State<DoctorArticlesPage> {
  late final repo = ArticlesRepository(widget.api);
  List<DoctorArticle> rows = [];
  bool loading = true, more = false;
  String? error;
  @override
  void initState() {
    super.initState();
    load();
  }

  Future<void> load({bool append = false}) async {
    if (mounted) {
      setState(() {
        loading = true;
        error = null;
      });
    }
    try {
      final next = await repo.list(offset: append ? rows.length : 0);
      if (mounted) {
        setState(() {
          rows = append ? [...rows, ...next] : next;
          more = next.length == 20;
        });
      }
    } on ApiException catch (e) {
      if (mounted) {
        setState(() => error = e.userMessage);
      }
    } catch (_) {
      if (mounted) {
        setState(() => error = 'Articles are unavailable. Please retry.');
      }
    } finally {
      if (mounted) {
        setState(() => loading = false);
      }
    }
  }

  Future<void> open([DoctorArticle? article]) async {
    await Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => _ArticleEditor(repo: repo, initial: article)));
    if (mounted) await load();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('My Articles'), actions: [
        IconButton(
            onPressed: loading ? null : () => load(),
            icon: const Icon(Icons.refresh))
      ]),
      floatingActionButton: FloatingActionButton.extended(
          onPressed: () => open(),
          icon: const Icon(Icons.edit_note),
          label: const Text('New article')),
      body: ListView(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 100),
          children: [
            const Text(
                'Write educational content related to your specialty. Publish directly to the patient feed.'),
            if (error != null) InlineError(error!, onRetry: () => load()),
            if (loading) const LinearProgressIndicator(),
            if (!loading && rows.isEmpty && error == null)
              const Padding(
                  padding: EdgeInsets.all(24),
                  child: Text('No articles yet. Create your first draft.')),
            ...rows.map((a) => DoctorCard(
                child: ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(a.title),
                    subtitle: Text('${a.status}\n${a.summary}'),
                    isThreeLine: true,
                    onTap: () => open(a)))),
            if (more)
              TextButton(
                  onPressed: loading ? null : () => load(append: true),
                  child: const Text('Load more')),
          ]));
}

class _ArticleEditor extends StatefulWidget {
  const _ArticleEditor({required this.repo, this.initial});
  final ArticlesRepository repo;
  final DoctorArticle? initial;
  @override
  State<_ArticleEditor> createState() => _EditorState();
}

class _EditorState extends State<_ArticleEditor> {
  final form = GlobalKey<FormState>();
  final creationKey = articleCreationKey();
  late DoctorArticle? article = widget.initial;
  late final title = TextEditingController(text: article?.title);
  late final summary = TextEditingController(text: article?.summary);
  late final body = TextEditingController(text: article?.body);
  late final references =
      TextEditingController(text: article?.references.join('\n'));
  bool busy = false;
  String? error, notice;
  @override
  void dispose() {
    title.dispose();
    summary.dispose();
    body.dispose();
    references.dispose();
    super.dispose();
  }

  Future<void> save(bool publish) async {
    if (busy || !form.currentState!.validate()) return;
    final refs = references.text
        .split('\n')
        .map((r) => r.trim())
        .where((r) => r.isNotEmpty)
        .toList();
    if (refs.length > 10 || refs.any((r) => r.runes.length > 500)) {
      setState(
          () => error = 'Include at most 10 references of 1–500 characters.');
      return;
    }
    setState(() {
      busy = true;
      error = null;
      notice = null;
    });
    try {
      final saved = await widget.repo
          .save(article: article, creationKey: creationKey, content: {
        'title': title.text.trim(),
        'summary': summary.text.trim(),
        'body': body.text.trim(),
        'references_list': refs
      });
      if (!mounted) return;
      setState(() => article = saved);
      if (publish) {
        final published = await widget.repo.transition(saved, publish: true);
        if (!mounted) return;
        setState(() => article = published);
      }
      if (mounted) {
        setState(() => notice = publish
            ? 'Published to the patient feed.'
            : article!.status == 'PUBLISHED'
                ? 'Published article updated.'
                : 'Draft saved.');
      }
    } on ApiException catch (e) {
      if (mounted) {
        setState(() => error = e.userMessage);
      }
    } catch (_) {
      if (mounted) {
        setState(() =>
            error = 'Could not save. Refresh the article before retrying.');
      }
    } finally {
      if (mounted) {
        setState(() => busy = false);
      }
    }
  }

  Future<void> reload() async {
    if (busy || article == null) return;
    setState(() {
      busy = true;
      error = null;
      notice = null;
    });
    try {
      final next = await widget.repo.get(article!.id);
      if (!mounted) return;
      setState(() {
        article = next;
        title.text = next.title;
        summary.text = next.summary;
        body.text = next.body;
        references.text = next.references.join('\n');
      });
    } on ApiException catch (e) {
      if (mounted) {
        setState(() => error = e.userMessage);
      }
    } catch (_) {
      if (mounted) {
        setState(() => error = 'Unable to reload article.');
      }
    } finally {
      if (mounted) {
        setState(() => busy = false);
      }
    }
  }

  Future<void> archive() async {
    if (busy || article == null) return;
    final confirmed = await showDialog<bool>(
        context: context,
        builder: (c) => AlertDialog(
                title: const Text('Archive article?'),
                content: const Text('It will leave the published feed.'),
                actions: [
                  TextButton(
                      onPressed: () => Navigator.pop(c, false),
                      child: const Text('Cancel')),
                  FilledButton(
                      onPressed: () => Navigator.pop(c, true),
                      child: const Text('Archive'))
                ]));
    if (confirmed != true || !mounted) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final next = await widget.repo.transition(article!, publish: false);
      if (mounted) {
        setState(() {
          article = next;
          notice = 'Article archived.';
        });
      }
    } on ApiException catch (e) {
      if (mounted) {
        setState(() => error = e.userMessage);
      }
    } catch (_) {
      if (mounted) {
        setState(() =>
            error = 'Unable to archive article. Refresh before retrying.');
      }
    } finally {
      if (mounted) {
        setState(() => busy = false);
      }
    }
  }

  Widget field(String label, TextEditingController controller, int max,
          {int lines = 1}) =>
      Padding(
          padding: const EdgeInsets.only(bottom: 16),
          child: TextFormField(
              controller: controller,
              readOnly: busy || !(article?.editable ?? true),
              maxLength: max,
              minLines: lines,
              maxLines: lines + 4,
              decoration: InputDecoration(labelText: label),
              validator: (value) => value == null || value.trim().isEmpty
                  ? '$label is required.'
                  : value.trim().runes.length > max
                      ? '$label exceeds $max characters.'
                      : null));
  @override
  Widget build(BuildContext context) => Scaffold(
      appBar: AppBar(title: const Text('Article'), actions: [
        if (article != null)
          IconButton(
              onPressed: busy ? null : reload,
              tooltip: 'Reload latest',
              icon: const Icon(Icons.refresh))
      ]),
      body: Form(
          key: form,
          child: ListView(padding: const EdgeInsets.all(18), children: [
            Text(
                '${article?.status ?? 'DRAFT'}${article == null ? '' : ' · ${article!.specialty}'}'),
            const SizedBox(height: 16),
            field('Title', title, 160),
            field('Summary', summary, 600, lines: 3),
            field('Article', body, 30000, lines: 12),
            TextField(
                controller: references,
                readOnly: busy || !(article?.editable ?? true),
                minLines: 2,
                maxLines: 6,
                decoration: const InputDecoration(
                    labelText: 'References (optional, one per line)')),
            const Padding(
                padding: EdgeInsets.symmetric(vertical: 16),
                child: Text(
                    'Use general educational content without identifiable patient information. Articles do not replace an individual consultation.')),
            if (error != null)
              Text(error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error)),
            if (notice != null) Text(notice!),
            if (busy) const LinearProgressIndicator(),
            if (article?.editable ?? true)
              OutlinedButton(
                  onPressed: busy ? null : () => save(false),
                  child: Text(article?.status == 'PUBLISHED'
                      ? 'Update published article'
                      : 'Save draft')),
            if (article?.publishable ?? true)
              FilledButton(
                  onPressed: busy ? null : () => save(true),
                  child: const Text('Publish')),
            if (article != null && article!.editable)
              TextButton(
                  onPressed: busy ? null : archive,
                  child: const Text('Archive')),
          ])));
}
