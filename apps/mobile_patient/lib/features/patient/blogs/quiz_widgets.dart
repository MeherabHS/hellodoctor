import 'package:flutter/material.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import 'quiz_models.dart';

class QuizAnswerOption extends StatelessWidget {
  const QuizAnswerOption(
      {super.key,
      required this.text,
      required this.selected,
      required this.onPressed});
  final String text;
  final bool selected;
  final VoidCallback? onPressed;
  @override
  Widget build(BuildContext context) => Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Semantics(
          selected: selected,
          child: SizedBox(
              width: double.infinity,
              child: OutlinedButton(
                  style: OutlinedButton.styleFrom(
                      minimumSize: const Size(48, 48),
                      alignment: Alignment.centerLeft,
                      backgroundColor: selected
                          ? PatientTokens.primaryLight
                          : PatientTokens.surface,
                      foregroundColor: PatientTokens.primaryDark,
                      side: BorderSide(
                          color: selected
                              ? PatientTokens.primary
                              : PatientTokens.border),
                      padding: const EdgeInsets.all(14)),
                  onPressed: onPressed,
                  child: Text(text)))));
}

class QuizQuestionCard extends StatelessWidget {
  const QuizQuestionCard(
      {super.key,
      required this.question,
      required this.selected,
      required this.disabled,
      required this.onSelect});
  final QuizQuestion question;
  final String? selected;
  final bool disabled;
  final ValueChanged<String> onSelect;
  @override
  Widget build(BuildContext context) {
    final language = Localizations.localeOf(context).languageCode;
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(question.textFor(language),
          style: Theme.of(context).textTheme.titleMedium),
      if (language == 'sw' && !question.useSw(language))
        Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(AppLocalizations.of(context).quizEnglishFallback)),
      const SizedBox(height: 12),
      for (var i = 0; i < question.options.length; i++)
        QuizAnswerOption(
            text:
                '${String.fromCharCode(65 + i)}. ${question.optionFor(question.options[i], language)}',
            selected: selected == question.options[i].id,
            onPressed:
                disabled ? null : () => onSelect(question.options[i].id)),
    ]);
  }
}

class QuizFeedback extends StatelessWidget {
  const QuizFeedback(
      {super.key, required this.feedback, required this.question});
  final QuizAnswerFeedback feedback;
  final QuizQuestion question;
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context),
        language = Localizations.localeOf(context).languageCode;
    final answer =
        question.options.firstWhere((o) => o.id == feedback.correctOptionId);
    return Semantics(
        liveRegion: true,
        child: Container(
            width: double.infinity,
            margin: const EdgeInsets.symmetric(vertical: 12),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
                color: feedback.correct
                    ? PatientTokens.primaryLight
                    : PatientTokens.warningLight,
                borderRadius: BorderRadius.circular(PatientTokens.radius)),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(feedback.correct ? l10n.quizCorrect : l10n.quizIncorrect,
                  style: const TextStyle(
                      fontWeight: FontWeight.w800, color: PatientTokens.text)),
              const SizedBox(height: 6),
              Text(
                  l10n.quizCorrectAnswer(question.optionFor(answer, language))),
              const SizedBox(height: 8),
              Text(l10n.quizExplanation,
                  style: const TextStyle(fontWeight: FontWeight.w700)),
              Text(
                  feedback
                      .explanationFor(question.useSw(language) ? 'sw' : 'en'),
                  style:
                      const TextStyle(color: PatientTokens.text, height: 1.5)),
            ])));
  }
}

class QuizCompletionSummary extends StatelessWidget {
  const QuizCompletionSummary(
      {super.key,
      required this.attempted,
      required this.total,
      required this.correct,
      required this.onRestart});
  final int attempted, total, correct;
  final VoidCallback onRestart;
  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return Semantics(
        liveRegion: true,
        child: PatientCard(
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Text(l10n.quizComplete,
              style: Theme.of(context).textTheme.titleLarge),
          const SizedBox(height: 12),
          Text(l10n.quizAttempted(attempted, total)),
          Text(l10n.quizCorrectCount(correct)),
          Text(l10n.quizIncorrectCount(attempted - correct)),
          Text(l10n.quizPercentage(total == 0 ? 0 : attempted * 100 ~/ total)),
          const SizedBox(height: 16),
          FilledButton(onPressed: onRestart, child: Text(l10n.quizRestart))
        ])));
  }
}
