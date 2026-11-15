import 'package:flutter_test/flutter_test.dart';
import 'package:hellodoctor_mobile_patient/features/patient/blogs/blogs_models.dart';

/// Doctor Web redesign (2026-11-14): Doctor articles may carry a cover, a topic and
/// a complete Swahili version, which the patient app must show like Admin content.
void main() {
  Map<String, dynamic> doctorArticle(
          {bool swahili = true, bool cover = true}) =>
      {
        'id': '00000000-0000-4000-8000-000000000001',
        'article_type': 'DOCTOR',
        'status': 'PUBLISHED',
        'author_name': 'Meherab Hossain',
        'specialty': 'General Practitioner',
        'title': 'Understanding PrEP',
        'summary': 'How PrEP protects you',
        'body': 'PrEP is a daily pill.',
        'references_list': <String>[],
        'published_at': '2026-11-14T08:00:00Z',
        'topic_key': 'PREP',
        if (swahili) ...{
          'title_sw': 'Kuelewa PrEP',
          'summary_sw': 'Jinsi PrEP inavyokulinda',
          'body_sw': 'PrEP ni kidonge cha kila siku.',
        },
        if (cover) 'cover_image_id': '00000000-0000-4000-8000-0000000000aa',
      };

  test('a doctor article shows its cover, topic and Swahili summary', () {
    final a = BlogArticleDto.fromJson(doctorArticle());
    expect(a.isAdmin, isFalse);
    expect(a.hasCover, isTrue);
    expect(a.topicKey, 'PREP');
    expect(a.titleFor('sw'), 'Kuelewa PrEP');
    expect(a.summaryFor('sw'), 'Jinsi PrEP inavyokulinda');
    expect(a.summaryFor('en'), 'How PrEP protects you');
  });

  test('without a Swahili version or cover the English article is used', () {
    final a =
        BlogArticleDto.fromJson(doctorArticle(swahili: false, cover: false));
    expect(a.hasCover, isFalse);
    expect(a.titleFor('sw'), 'Understanding PrEP');
    expect(a.summaryFor('sw'), 'How PrEP protects you');
  });
}
