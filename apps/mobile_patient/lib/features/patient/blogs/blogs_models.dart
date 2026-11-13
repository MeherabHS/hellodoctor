import 'quiz_models.dart';

typedef Json = Map<String, dynamic>;

class BlogCursor {
  const BlogCursor(this.time, this.id);
  final String time, id;
  factory BlogCursor.fromJson(Json json) {
    final time = json['before_time']?.toString() ?? '';
    final id = json['before_id']?.toString() ?? '';
    if (DateTime.tryParse(time) == null ||
        !RegExp(r'^[0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12}$')
            .hasMatch(id)) {
      throw const FormatException('Invalid article cursor.');
    }
    return BlogCursor(time, id);
  }
}

class BlogPage {
  const BlogPage(this.items, this.next);
  final List<BlogArticleDto> items;
  final BlogCursor? next;
}

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
    this.topicKey,
    this.videoUrl,
    this.standaloneQuiz = false,
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

  /// The HIV Prevention & Sexual Health topic of this entry's category
  /// (HIV_BASICS, PREP, ...), or null.
  final String? topicKey;

  /// Canonical YouTube watch URL when this entry is a video, else null.
  final Uri? videoUrl;
  bool get isVideo => videoUrl != null;

  /// A quiz that is not attached to a blog article (Admin Quiz Manager);
  /// its body is a short introduction.
  final bool standaloneQuiz;

  /// Estimated reading time at 200 words a minute, at least one minute.
  int readMinutesFor(String language) {
    final words = bodyFor(language)
        .split(RegExp(r'\s+'))
        .where((w) => w.isNotEmpty)
        .length;
    return words <= 200 ? 1 : (words / 200).ceil();
  }

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
      topicKey:
          admin && j['topic_key'] is String ? j['topic_key'] as String : null,
      videoUrl: admin ? youTubeWatchUri(j['video_url']) : null,
      standaloneQuiz: admin && j['standalone_quiz'] == true,
    );
  }
}

/// Accepts only the backend's canonical YouTube watch URL, so a tampered or
/// unexpected value can never make the app open another site.
Uri? youTubeWatchUri(dynamic value) {
  if (value is! String) return null;
  return RegExp(r'^https://www\.youtube\.com/watch\?v=[A-Za-z0-9_-]{11}$')
          .hasMatch(value)
      ? Uri.parse(value)
      : null;
}
