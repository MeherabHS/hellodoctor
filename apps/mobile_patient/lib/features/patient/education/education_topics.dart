import 'package:flutter/material.dart';

import '../../../l10n/app_localizations.dart';

/// The six HIV Prevention & Sexual Health topics (reference panel 2 and the
/// product owner's per-topic flows, `docs/design/education/`).
///
/// [apiKey] is the backend's `article_categories.topic_key`: an Admin links a
/// category to a topic, and the topic page lists that category's published
/// articles. Colours were sampled from the per-topic empty-state references
/// (941 px wide, about 2.29 px per dp); see `docs/patient-ui/EDUCATION_TOPICS.md`.
enum EducationTopic {
  hivBasics(
      apiKey: 'HIV_BASICS',
      asset: 'hiv_basics',
      thumbnail: 'topic_hiv_basics',
      bannerColor: Color(0xFFE4FAF7),
      bannerArtEdge: Color(0xFFD2F7F0),
      bannerArtWidth: 152,
      emptyColor: Color(0xFFEAFBF3)),
  prep(
      apiKey: 'PREP',
      asset: 'prep',
      thumbnail: 'topic_prep',
      bannerColor: Color(0xFFE3F9F7),
      bannerArtEdge: Color(0xFFCDF6F2),
      bannerArtWidth: 176,
      emptyColor: Color(0xFFE7FAF8)),
  condoms(
      apiKey: 'CONDOMS',
      asset: 'condoms',
      thumbnail: 'topic_condoms',
      bannerColor: Color(0xFFFFE5EE),
      bannerArtEdge: Color(0xFFFEE5EB),
      bannerArtWidth: 176,
      emptyColor: Color(0xFFFDF1F5)),
  srh(
      apiKey: 'SRH',
      asset: 'srh',
      thumbnail: 'topic_srh',
      bannerColor: Color(0xFFF4EBFC),
      bannerArtEdge: Color(0xFFEDE3FC),
      bannerArtWidth: 148,
      emptyColor: Color(0xFFF6F0FE)),
  stigma(
      apiKey: 'STIGMA_DISCLOSURE',
      asset: 'stigma',
      thumbnail: 'topic_stigma',
      bannerColor: Color(0xFFF5E9FD),
      bannerArtEdge: Color(0xFFF2DFFD),
      bannerArtWidth: 182,
      emptyColor: Color(0xFFF4EBFE)),
  relationships(
      apiKey: 'HEALTHY_RELATIONSHIPS',
      asset: 'relationships',
      thumbnail: 'topic_relationships',
      bannerColor: Color(0xFFE7FCED),
      bannerArtEdge: Color(0xFFDDFAE6),
      bannerArtWidth: 193,
      emptyColor: Color(0xFFE9FDF2));

  const EducationTopic({
    required this.apiKey,
    required this.asset,
    required this.thumbnail,
    required this.bannerColor,
    required this.bannerArtEdge,
    required this.bannerArtWidth,
    required this.emptyColor,
  });

  final String apiKey;
  final String asset;

  /// Existing 104 dp list artwork in `assets/illustrations/`.
  final String thumbnail;
  final Color bannerColor, bannerArtEdge, emptyColor;

  /// Width of the banner artwork in dp at the reference scale.
  final double bannerArtWidth;

  String get bannerAsset => 'assets/education/banner_$asset.png';
  String get emptyAsset => 'assets/education/empty_$asset.png';

  static EducationTopic? fromApiKey(String? key) {
    for (final t in values) {
      if (t.apiKey == key) return t;
    }
    return null;
  }

  String title(AppLocalizations l) => switch (this) {
        hivBasics => l.eduTopicHivBasics,
        prep => l.eduTopicPrep,
        condoms => l.eduTopicCondoms,
        srh => l.eduTopicSrh,
        stigma => l.eduTopicStigma,
        relationships => l.eduTopicRelationships,
      };

  /// Short line on the hub list.
  String listDescription(AppLocalizations l) => switch (this) {
        hivBasics => l.eduTopicHivBasicsDesc,
        prep => l.eduTopicPrepDesc,
        condoms => l.eduTopicCondomsDesc,
        srh => l.eduTopicSrhDesc,
        stigma => l.eduTopicStigmaDesc,
        relationships => l.eduTopicRelationshipsDesc,
      };

  /// Longer line on the topic page banner.
  String bannerDescription(AppLocalizations l) => switch (this) {
        hivBasics => l.eduBannerHivBasics,
        prep => l.eduBannerPrep,
        condoms => l.eduBannerCondoms,
        srh => l.eduBannerSrh,
        stigma => l.eduBannerStigma,
        relationships => l.eduBannerRelationships,
      };

  String emptyBody(AppLocalizations l) => switch (this) {
        hivBasics => l.eduNoArticlesHivBasics,
        prep => l.eduNoArticlesPrep,
        condoms => l.eduNoArticlesCondoms,
        srh => l.eduNoArticlesSrh,
        stigma => l.eduNoArticlesStigma,
        relationships => l.eduNoArticlesRelationships,
      };
}
