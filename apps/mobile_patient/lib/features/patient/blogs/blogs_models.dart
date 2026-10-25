import 'quiz_models.dart';

typedef Json = Map<String, dynamic>;

String _textOf(dynamic value, [String fallback = '']) =>
    value == null ? fallback : value.toString();

/// Unified published article contract. Existing Doctor responses remain valid;
/// Admin content adds Markdown, bilingual fields, categories and private cover IDs.
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
    this.articleType = 'DOCTOR',
    this.contentFormat = 'plain',
    this.titleSw = '',
    this.bodySw = '',
    this.categoryEn = '',
    this.categorySw = '',
    this.coverImageId,
    this.quiz,
  });
  final String id, authorName, specialty, title, summary, body;
  final List<String> referencesList;
  final DateTime? publishedAt;
  final String articleType,
      contentFormat,
      titleSw,
      bodySw,
      categoryEn,
      categorySw;
  final String? coverImageId;
  final QuizPublication? quiz;
  bool get isAdmin => articleType == 'ADMIN';
  bool get hasCover => isAdmin && coverImageId != null;
  bool _sw(String language) =>
      language == 'sw' && titleSw.trim().isNotEmpty && bodySw.trim().isNotEmpty;
  String titleFor(String language) => _sw(language) ? titleSw : title;
  String bodyFor(String language) => _sw(language) ? bodySw : body;
  String summaryFor(String language) {
    if (!isAdmin) return summary;
    final value = bodyFor(language);
    return value.length > 300 ? value.substring(0, 300) : value;
  }

  String categoryFor(String language) =>
      language == 'sw' && categorySw.isNotEmpty ? categorySw : categoryEn;

  factory BlogArticleDto.fromJson(Json j) {
    if (j['status'] != null && j['status'] != 'PUBLISHED') {
      throw const FormatException('Unpublished article response rejected.');
    }
    final admin = j['article_type'] == 'ADMIN';
    final cover = _textOf(j['cover_image_id']);
    return BlogArticleDto(
      id: _textOf(j['id']),
      authorName: admin ? 'AfyaMkononi Editorial' : _textOf(j['author_name']),
      specialty: admin ? 'Editorial' : _textOf(j['specialty']),
      title: _textOf(j['title']),
      summary: _textOf(j['summary']),
      body: _textOf(j['body']),
      referencesList: (j['references_list'] as List? ?? const [])
          .map((e) => e.toString())
          .toList(),
      publishedAt: DateTime.tryParse(_textOf(j['published_at']))?.toLocal(),
      articleType: admin ? 'ADMIN' : 'DOCTOR',
      contentFormat:
          admin && j['content_format'] == 'markdown' ? 'markdown' : 'plain',
      titleSw: _textOf(j['title_sw']),
      bodySw: _textOf(j['body_sw']),
      categoryEn: _textOf(j['category_name_en']),
      categorySw: _textOf(j['category_name_sw']),
      coverImageId:
          RegExp(r'^[0-9a-fA-F-]{36}$').hasMatch(cover) ? cover : null,
      quiz: admin ? QuizPublication.parse(j['quiz']) : null,
    );
  }
}
