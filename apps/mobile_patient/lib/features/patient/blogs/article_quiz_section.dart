import 'package:flutter/material.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'quiz_controller.dart';
import 'quiz_models.dart';
import 'quiz_repository.dart';
import 'quiz_widgets.dart';

class ArticleQuizSection extends StatefulWidget {
  const ArticleQuizSection(
      {super.key,
      required this.articleId,
      required this.publication,
      required this.repository,
      required this.onRefreshArticle});
  final String articleId;
  final QuizPublication publication;
  final ArticleQuizRepository repository;
  final VoidCallback onRefreshArticle;
  @override
  State<ArticleQuizSection> createState() => _ArticleQuizSectionState();
}

class _ArticleQuizSectionState extends State<ArticleQuizSection> {
  late ArticleQuizController _controller;
  void _create() {
    _controller = ArticleQuizController(
        widget.repository, widget.articleId, widget.publication);
  }

  @override
  void initState() {
    super.initState();
    _create();
  }

  @override
  void didUpdateWidget(covariant ArticleQuizSection old) {
    super.didUpdateWidget(old);
    if (old.articleId != widget.articleId ||
        old.publication.revisionId != widget.publication.revisionId ||
        old.repository != widget.repository) {
      _controller.dispose();
      _create();
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final c = _controller, l10n = AppLocalizations.of(context);
        final failure = switch (c.failure) {
          QuizFailure.answer => l10n.quizAnswerError,
          QuizFailure.session => l10n.quizSessionExpired,
          QuizFailure.rateLimited => l10n.quizRateLimited,
          _ => l10n.quizLoadError
        };
        if (c.phase == QuizPhase.complete) {
          return QuizCompletionSummary(
              attempted: c.attempted,
              total: c.total,
              correct: c.correct,
              onRestart: () => c.start());
        }
        return PatientCard(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(l10n.quizTitle, style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 10),
          if (c.phase == QuizPhase.changed ||
              c.phase == QuizPhase.unavailable) ...[
            Semantics(
                liveRegion: true,
                child: Text(c.phase == QuizPhase.changed
                    ? l10n.quizUpdated
                    : l10n.quizUnavailable)),
            TextButton(
                onPressed: widget.onRefreshArticle,
                child: Text(l10n.quizRefreshArticle)),
          ] else if (c.phase == QuizPhase.loading) ...[
            Semantics(liveRegion: true, child: Text(l10n.quizLoading)),
            if (!MediaQuery.disableAnimationsOf(context))
              const Padding(
                  padding: EdgeInsets.only(top: 12),
                  child: LinearProgressIndicator()),
          ] else if (c.phase == QuizPhase.introduction) ...[
            Text(l10n.quizIntro),
            Text(l10n.quizQuestionCount(c.total)),
            const SizedBox(height: 12),
            FilledButton(
                onPressed: c.busy ? null : () => c.start(),
                child:
                    Text(c.failure == null ? l10n.quizStart : l10n.quizRetry)),
          ] else ...[
            Text(l10n.quizPosition(c.index + 1, c.total)),
            const SizedBox(height: 8),
            LinearProgressIndicator(
                value: c.attempted / c.total,
                semanticsLabel: l10n.quizAttempted(c.attempted, c.total)),
            const SizedBox(height: 16),
            QuizQuestionCard(
                question: c.question,
                selected: c.selected,
                disabled: c.busy || c.feedback != null,
                onSelect: c.select),
            if (c.feedback != null) ...[
              QuizFeedback(feedback: c.feedback!, question: c.question),
              FilledButton(
                  onPressed: c.next,
                  child: Text(
                      c.index + 1 == c.total ? l10n.quizFinish : l10n.quizNext))
            ] else
              FilledButton(
                  onPressed:
                      c.busy || c.selected == null ? null : () => c.submit(),
                  child: Text(c.busy ? l10n.quizLoading : l10n.quizSubmit)),
          ],
          if (c.failure != null)
            Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Semantics(liveRegion: true, child: Text(failure))),
        ]));
      });
}
