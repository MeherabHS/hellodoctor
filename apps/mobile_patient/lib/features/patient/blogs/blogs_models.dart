typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();

/// Mirrors `GET /articles` / `GET /articles/:id`
/// (`article_handlers.rs`'s `DoctorArticle`, plain snake-case JSON). Both
/// endpoints only ever return `status == 'PUBLISHED'` rows for the generic
/// (non-author) caller — confirmed directly in `article_repo::list`/`find`'s
/// SQL. There is no `category` or "read time" field on this model — the
/// prototype's `view-13` demo badges ("Nutrition & Care", "5 min read") are
/// static mockup text with no backing data, so this app does not fabricate
/// them.
class BlogArticleDto {
  const BlogArticleDto({
    required this.id,
    required this.authorName,
    required this.specialty,
    required this.title,
    required this.summary,
    required this.body,
    required this.referencesList,
    required this.publishedAt,
  });
  final String id, authorName, specialty, title, summary, body;
  final List<String> referencesList;
  final DateTime? publishedAt;

  factory BlogArticleDto.fromJson(Json j) => BlogArticleDto(
        id: _textOf(j['id']),
        authorName: _textOf(j['author_name']),
        specialty: _textOf(j['specialty']),
        title: _textOf(j['title']),
        summary: _textOf(j['summary']),
        body: _textOf(j['body']),
        referencesList: (j['references_list'] as List? ?? const [])
            .map((e) => e.toString())
            .toList(),
        publishedAt: DateTime.tryParse(_textOf(j['published_at']))?.toLocal(),
      );
}
