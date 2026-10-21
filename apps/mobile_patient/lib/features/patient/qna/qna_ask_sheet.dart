import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../../core/theme/tokens.dart';
import '../../../l10n/app_localizations.dart';
import '../profile/family_profiles_cubit.dart';
import 'qna_cubit.dart';
import 'qna_models.dart';

Future<void> showAskQuestionSheet(
  BuildContext context, {
  required QnaCubit cubit,
  required FamilyProfilesCubit familyProfilesCubit,
}) async {
  cubit.resetSubmitState();
  await Navigator.of(context).push<void>(MaterialPageRoute(
      builder: (_) => AskQuestionPage(
          cubit: cubit, familyProfilesCubit: familyProfilesCubit)));
  if (!cubit.isClosed && cubit.state.submitted) await cubit.load();
}

class AskQuestionPage extends StatefulWidget {
  const AskQuestionPage(
      {super.key, required this.cubit, required this.familyProfilesCubit});
  final QnaCubit cubit;
  final FamilyProfilesCubit familyProfilesCubit;
  @override
  State<AskQuestionPage> createState() => _AskQuestionPageState();
}

class _AskQuestionPageState extends State<AskQuestionPage> {
  final _form = GlobalKey<FormState>();
  final _question = TextEditingController();
  String _category = qnaCategories.first;
  String? _profileId;
  @override
  void initState() {
    super.initState();
    _profileId = widget.familyProfilesCubit.state.activeProfileId;
  }

  @override
  void dispose() {
    _question.dispose();
    super.dispose();
  }

  void _submit() {
    if (!_form.currentState!.validate()) return;
    FocusScope.of(context).unfocus();
    widget.cubit.submit(
        questionText: _question.text,
        category: _category,
        profileId: _profileId);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final profiles = widget.familyProfilesCubit.state.profiles;
    return Scaffold(
      appBar: AppBar(title: Text(l.qnaAskAction)),
      body: SafeArea(
          child: BlocBuilder<QnaCubit, QnaState>(
              bloc: widget.cubit,
              builder: (context, state) {
                if (state.submitted) {
                  return Padding(
                      padding: const EdgeInsets.all(24),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            const SizedBox(height: 32),
                            const Icon(Icons.check_circle_outline_rounded,
                                color: PatientTokens.primary, size: 40),
                            const SizedBox(height: 20),
                            Text(l.qnaSuccessTitle,
                                style:
                                    Theme.of(context).textTheme.headlineSmall,
                                textAlign: TextAlign.center),
                            const SizedBox(height: 12),
                            Text(l.qnaSuccessBody, textAlign: TextAlign.center),
                            const SizedBox(height: 28),
                            FilledButton(
                                onPressed: () => Navigator.pop(context),
                                child: Text(l.qnaBackToFeed,
                                    textAlign: TextAlign.center)),
                          ]));
                }
                return Form(
                    key: _form,
                    child:
                        ListView(padding: const EdgeInsets.all(24), children: [
                      Text(l.qnaScreenSubtitle),
                      const SizedBox(height: 24),
                      if (profiles.length > 1) ...[
                        Text(l.qnaAskingAs,
                            style: Theme.of(context).textTheme.titleSmall),
                        const SizedBox(height: 8),
                        DropdownButtonFormField<String>(
                            initialValue: _profileId ?? 'self',
                            isExpanded: true,
                            items: profiles
                                .map((p) => DropdownMenuItem(
                                    value: p.isSelf ? 'self' : p.id,
                                    child: Text(p.fullName,
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis)))
                                .toList(),
                            onChanged: state.submitting
                                ? null
                                : (v) => setState(
                                    () => _profileId = v == 'self' ? null : v)),
                        const SizedBox(height: 24),
                      ],
                      Text(l.qnaSelectTopic,
                          style: Theme.of(context).textTheme.titleSmall),
                      const SizedBox(height: 8),
                      DropdownButtonFormField<String>(
                          initialValue: _category,
                          isExpanded: true,
                          items: qnaCategories
                              .map((c) => DropdownMenuItem(
                                  value: c, child: Text(_categoryLabel(l, c))))
                              .toList(),
                          onChanged: state.submitting
                              ? null
                              : (v) => setState(
                                  () => _category = v ?? qnaCategories.first)),
                      const SizedBox(height: 24),
                      Text(l.qnaQuestionLabel,
                          style: Theme.of(context).textTheme.titleSmall),
                      const SizedBox(height: 8),
                      TextFormField(
                          key: const Key('qna-question'),
                          controller: _question,
                          enabled: !state.submitting,
                          minLines: 5,
                          maxLines: 10,
                          maxLength: 4000,
                          keyboardType: TextInputType.multiline,
                          decoration: InputDecoration(
                              hintText: l.qnaHint, alignLabelWithHint: true),
                          validator: (v) => v == null ||
                                  v.trim().isEmpty ||
                                  v.trim().runes.length > 4000
                              ? l.qnaValidation
                              : null),
                      const SizedBox(height: 12),
                      Text(l.qnaPrivacyGuide),
                      const SizedBox(height: 12),
                      Row(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            const Icon(Icons.shield_outlined,
                                size: 18, color: PatientTokens.primary),
                            const SizedBox(width: 8),
                            Expanded(child: Text(l.qnaAnonymousNotice))
                          ]),
                      if (state.submitError != null)
                        Padding(
                            padding: const EdgeInsets.only(top: 16),
                            child: Text(state.submitError!,
                                style: const TextStyle(
                                    color: PatientTokens.danger))),
                      const SizedBox(height: 28),
                      FilledButton(
                          key: const Key('qna-submit'),
                          onPressed: state.submitting ? null : _submit,
                          child: Text(
                              state.submitting
                                  ? l.qnaSubmitting
                                  : l.qnaSubmitAction,
                              textAlign: TextAlign.center)),
                    ]));
              })),
    );
  }

  String _categoryLabel(AppLocalizations l, String c) => switch (c) {
        'General Health' => l.qnaTopicGeneral,
        'Fevers & Infections' => l.qnaTopicFevers,
        'Child Health' => l.qnaTopicChild,
        'Diet & Diabetes' => l.qnaTopicDiet,
        'Skin & Allergy' => l.qnaTopicSkin,
        _ => l.qnaTopicGeneral,
      };
}
