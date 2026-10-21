import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/theme/tokens.dart';
import '../../../core/widgets/patient_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../profile/family_profiles_cubit.dart';
import 'qna_ask_sheet.dart';
import 'qna_cubit.dart';
import 'qna_models.dart';
import 'qna_repository.dart';

class QnaPage extends StatefulWidget {
  const QnaPage(
      {super.key, required this.repo, required this.familyProfilesCubit});
  final QnaRepository repo;
  final FamilyProfilesCubit familyProfilesCubit;
  @override
  State<QnaPage> createState() => _QnaPageState();
}

class _QnaPageState extends State<QnaPage> {
  late final QnaCubit _cubit;
  @override
  void initState() {
    super.initState();
    _cubit = QnaCubit(widget.repo)..load();
  }

  @override
  void dispose() {
    _cubit.close();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: PatientTokens.surface,
        body: SafeArea(
            child: BlocBuilder<QnaCubit, QnaState>(
                bloc: _cubit,
                builder: (context, state) => QnaFeed(
                    state: state,
                    onRefresh: _cubit.load,
                    onAsk: () => showAskQuestionSheet(context,
                        cubit: _cubit,
                        familyProfilesCubit: widget.familyProfilesCubit)))),
      );
}

class QnaFeed extends StatelessWidget {
  const QnaFeed(
      {super.key,
      required this.state,
      required this.onRefresh,
      required this.onAsk});
  final QnaState state;
  final Future<void> Function() onRefresh;
  final VoidCallback onAsk;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final questions =
        state.questions.where((q) => !q.isInternalFixture).toList();
    return RefreshIndicator(
        onRefresh: onRefresh,
        child: ListView(
          physics: const AlwaysScrollableScrollPhysics(),
          padding: const EdgeInsets.fromLTRB(24, 24, 24, 32),
          children: [
            Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Expanded(
                  child: Text(l.qnaScreenTitle,
                      style: Theme.of(context).textTheme.headlineSmall)),
              IconButton(
                  tooltip: l.retryButton,
                  onPressed: onRefresh,
                  icon: const Icon(Icons.refresh_rounded))
            ]),
            const SizedBox(height: 8),
            Text(l.qnaScreenSubtitle),
            const SizedBox(height: 24),
            FilledButton.icon(
                onPressed: onAsk,
                icon: const Icon(Icons.edit_outlined, size: 18),
                label: Text(l.qnaAskAction, textAlign: TextAlign.center)),
            const SizedBox(height: 32),
            Text(l.qnaRecent, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 8),
            if (state.loading && questions.isEmpty)
              const SkeletonBlock(height: 160),
            if (state.error != null)
              InlineError(state.error!, onRetry: onRefresh),
            if (!state.loading && questions.isEmpty && state.error == null)
              EmptyState(
                  icon: Icons.forum_outlined,
                  title: l.qnaEmptyTitle,
                  message: l.qnaEmptyBody),
            for (final q in questions) QuestionArticle(question: q),
          ],
        ));
  }
}

class QuestionArticle extends StatelessWidget {
  const QuestionArticle(
      {super.key, required this.question, this.expanded = false});
  final AnsweredQuestionDto question;
  final bool expanded;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    if (question.isInternalFixture) return const SizedBox.shrink();
    final category = switch (question.displayCategory) {
      'Fevers & Infections' => l.qnaTopicFevers,
      'Child Health' => l.qnaTopicChild,
      'Diet & Diabetes' => l.qnaTopicDiet,
      'Skin & Allergy' => l.qnaTopicSkin,
      _ => l.qnaTopicGeneral,
    };
    return Padding(
        padding: const EdgeInsets.symmetric(vertical: 20),
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
          Wrap(spacing: 8, runSpacing: 4, children: [
            Text(category,
                style: const TextStyle(
                    color: PatientTokens.textSub, fontSize: 13)),
            Text(l.qnaAnonymous,
                style:
                    const TextStyle(color: PatientTokens.textSub, fontSize: 13))
          ]),
          const SizedBox(height: 12),
          Text(question.questionText,
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 16),
          if (question.isAnswered) ...[
            const Divider(),
            const SizedBox(height: 16),
            Text(l.qnaAnsweredBy,
                style: const TextStyle(
                    color: PatientTokens.textSub, fontSize: 12)),
            const SizedBox(height: 4),
            Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Expanded(
                  child: Text(question.doctorDisplayName,
                      style: Theme.of(context).textTheme.titleSmall)),
              if (question.answeringDoctorVerified)
                Icon(Icons.verified_outlined,
                    size: 18,
                    color: PatientTokens.primary,
                    semanticLabel: l.profileVerified)
            ]),
            Text(question.answeringDoctorSpecialty,
                style: const TextStyle(
                    fontSize: 13, color: PatientTokens.textSub)),
            const SizedBox(height: 12),
            Text(question.answerText,
                maxLines: expanded ? null : 4,
                overflow:
                    expanded ? TextOverflow.visible : TextOverflow.ellipsis,
                style: Theme.of(context).textTheme.bodyLarge),
            if (!expanded)
              Align(
                  alignment: Alignment.centerRight,
                  child: TextButton(
                      onPressed: () => Navigator.push(
                          context,
                          MaterialPageRoute<void>(
                              builder: (_) => Scaffold(
                                  appBar: AppBar(title: Text(l.qnaScreenTitle)),
                                  body: SafeArea(
                                      child: SingleChildScrollView(
                                          padding: const EdgeInsets.all(24),
                                          child: QuestionArticle(
                                              question: question,
                                              expanded: true)))))),
                      child: Text(l.qnaReadFull))),
          ] else
            Text(l.qnaAwaiting,
                style: const TextStyle(
                    color: PatientTokens.primary,
                    fontSize: 14,
                    fontWeight: FontWeight.w600)),
          const SizedBox(height: 20),
          const Divider(),
        ]));
  }
}
