import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:url_launcher/url_launcher.dart';
import '../../../core/theme/tokens.dart';
import 'blogs_repository.dart';

Uri? safeArticleLink(String? value) {
  if (value == null || RegExp(r'[\x00-\x20\x7f\\]').hasMatch(value)) {
    return null;
  }
  final uri = Uri.tryParse(value);
  return uri != null &&
          ['https', 'http'].contains(uri.scheme.toLowerCase()) &&
          uri.host.isNotEmpty &&
          uri.userInfo.isEmpty
      ? uri
      : null;
}

class ArticleMarkdown extends StatelessWidget {
  const ArticleMarkdown({super.key, required this.body});
  final String body;
  @override
  Widget build(BuildContext context) => MarkdownBody(
      data: body,
      // Images (including file/resource/data URLs) are never fetched from Markdown.
      imageBuilder: (_, __, ___) => const SizedBox.shrink(),
      onTapLink: (_, href, __) async {
        final uri = safeArticleLink(href);
        if (uri == null) return;
        try {
          await launchUrl(uri, mode: LaunchMode.externalApplication);
        } catch (_) {/* URL launch failure leaves the article readable. */}
      },
      // Unordered list items read as green ticks, as in the product owner's
      // article designs (`docs/design/education/flow-*.png`); numbered lists
      // keep their numbers.
      bulletBuilder: (p) => p.style == BulletStyle.unorderedList
          ? const Padding(
              padding: EdgeInsets.only(top: 2),
              child: Icon(Icons.check_circle_rounded,
                  size: 18, color: _checkGreen))
          : Text('${p.index + 1}.',
              style: const TextStyle(
                  fontSize: 14.5,
                  fontWeight: FontWeight.w700,
                  color: _sectionGreen)),
      styleSheet: MarkdownStyleSheet.fromTheme(Theme.of(context)).copyWith(
        p: const TextStyle(fontSize: 14.5, color: _bodyInk, height: 1.45),
        listBullet: const TextStyle(fontSize: 14.5, color: _bodyInk),
        listIndent: 28,
        blockSpacing: 12,
        h1: const TextStyle(
            fontSize: 21, fontWeight: FontWeight.w800, color: _sectionGreen),
        h2: const TextStyle(
            fontSize: 19, fontWeight: FontWeight.w800, color: _sectionGreen),
        h3: const TextStyle(
            fontSize: 16.5, fontWeight: FontWeight.w800, color: _sectionGreen),
        a: const TextStyle(
            color: PatientTokens.primary, decoration: TextDecoration.underline),
      ));
}

// Sampled from the article designs (`docs/design/education/flow-*.png`).
const _sectionGreen = Color(0xFF0F7B2B);
const _checkGreen = Color(0xFF1E9638);
const _bodyInk = Color(0xFF0B2A3F);

/// Uses the existing authenticated API transport, never untrusted response URLs or a disk cache.
class ArticleCover extends StatefulWidget {
  const ArticleCover(
      {super.key,
      required this.articleId,
      required this.mediaId,
      required this.repo});
  final String articleId, mediaId;
  final BlogsRepository repo;
  @override
  State<ArticleCover> createState() => _ArticleCoverState();
}

class _ArticleCoverState extends State<ArticleCover> {
  late Future<List<int>> _bytes;
  @override
  void initState() {
    super.initState();
    _bytes = widget.repo.cover(widget.articleId);
  }

  @override
  void didUpdateWidget(covariant ArticleCover oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.articleId != widget.articleId ||
        oldWidget.mediaId != widget.mediaId) {
      _bytes = widget.repo.cover(widget.articleId);
    }
  }

  @override
  Widget build(BuildContext context) => FutureBuilder<List<int>>(
      future: _bytes,
      builder: (context, snapshot) {
        if (snapshot.hasError) return const SizedBox.shrink();
        if (!snapshot.hasData) {
          return const SizedBox(
              height: 150, child: Center(child: CircularProgressIndicator()));
        }
        return Padding(
            padding: const EdgeInsets.only(bottom: 14),
            child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: Image.memory(Uint8List.fromList(snapshot.data!),
                    height: 150,
                    width: double.infinity,
                    fit: BoxFit.cover,
                    cacheWidth: 900,
                    errorBuilder: (_, __, ___) => const SizedBox.shrink())));
      });
}
