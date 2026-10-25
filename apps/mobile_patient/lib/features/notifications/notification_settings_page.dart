// ignore_for_file: use_build_context_synchronously
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/l10n/locale_controller.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/patient_widgets.dart';
import '../../l10n/app_localizations.dart';
import 'notification_cubit.dart';
import 'notification_models.dart';

/// Milestone 3 task 11: a simple notification-settings screen using this
/// app's existing settings patterns (`ErrorPanel`'s loading/error shape,
/// `PatientCard` rows, `showTimePicker`). Unlike the Doctor app's equivalent
/// screen, this one also surfaces the Patient-only EN/SW language toggle
/// (CLAUDE.md scopes the bilingual toggle to the Patient interface) so a
/// language change here immediately syncs `users.preferred_language` to the
/// backend via `LocaleController`'s bound sync hook.
class NotificationSettingsPage extends StatefulWidget {
  const NotificationSettingsPage({super.key});
  @override
  State<NotificationSettingsPage> createState() =>
      _NotificationSettingsPageState();
}

class _NotificationSettingsPageState extends State<NotificationSettingsPage> {
  bool _loading = true;
  bool _saving = false;
  bool _hasError = false;
  NotificationPreferencesDto? _prefs;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _hasError = false;
    });
    try {
      final prefs = await context.read<NotificationCubit>().repo.getPreferences();
      if (!mounted) return;
      setState(() {
        _prefs = prefs;
        _loading = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _hasError = true;
        _loading = false;
      });
    }
  }

  Future<void> _save(NotificationPreferencesDto next) async {
    final previous = _prefs;
    setState(() {
      _prefs = next;
      _saving = true;
    });
    try {
      final repo = context.read<NotificationCubit>().repo;
      final saved = await repo.updatePreferences(
        operationalEnabled: next.operationalEnabled,
        engagementEnabled: next.engagementEnabled,
        quietHoursEnabled: next.quietHoursEnabled,
        quietHoursStartLocal: next.quietHoursStartLocal,
        quietHoursEndLocal: next.quietHoursEndLocal,
        greetingNotificationsEnabled: next.greetingNotificationsEnabled,
        religiousGreetingsEnabled: next.religiousGreetingsEnabled,
      );
      if (!mounted) return;
      setState(() {
        _prefs = saved;
        _saving = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _prefs = previous;
        _saving = false;
      });
      final l10n = AppLocalizations.of(context);
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.notificationSettingsSaveError)));
    }
  }

  Future<void> _pickTime(
      {required bool isStart, required NotificationPreferencesDto current}) async {
    final raw = isStart ? current.quietHoursStartLocal : current.quietHoursEndLocal;
    final parts = raw.split(':');
    final initial = TimeOfDay(
        hour: int.tryParse(parts.isNotEmpty ? parts[0] : '') ?? 22,
        minute: int.tryParse(parts.length > 1 ? parts[1] : '') ?? 0);
    final picked = await showTimePicker(context: context, initialTime: initial);
    if (!mounted || picked == null) return;
    final formatted =
        '${picked.hour.toString().padLeft(2, '0')}:${picked.minute.toString().padLeft(2, '0')}';
    await _save(isStart
        ? current.copyWith(quietHoursStartLocal: formatted)
        : current.copyWith(quietHoursEndLocal: formatted));
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final prefs = _prefs;
    return Scaffold(
        backgroundColor: PatientTokens.bg,
        appBar: AppBar(title: Text(l10n.notificationSettingsPageTitle)),
        body: _loading
            ? const Center(child: CircularProgressIndicator())
            : _hasError
                ? ErrorPanel(l10n.notificationSettingsLoadError, onRetry: _load)
                : prefs == null
                    ? const SizedBox.shrink()
                    : ListView(padding: const EdgeInsets.all(18), children: [
                        const _LanguageSection(),
                        const SizedBox(height: 12),
                        PatientCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: Text(l10n.notificationSettingsRoutineTitle,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text)),
                                subtitle: Text(
                                    l10n.notificationSettingsRoutineSubtitle,
                                    style: const TextStyle(
                                        fontSize: 12, color: PatientTokens.textSub)),
                                value: prefs.operationalEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(
                                        prefs.copyWith(operationalEnabled: value)))),
                        const SizedBox(height: 10),
                        PatientCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: Text(l10n.notificationSettingsAnnouncementsTitle,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text)),
                                subtitle: Text(
                                    l10n.notificationSettingsAnnouncementsSubtitle,
                                    style: const TextStyle(
                                        fontSize: 12, color: PatientTokens.textSub)),
                                value: prefs.engagementEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(
                                        prefs.copyWith(engagementEnabled: value)))),
                        const SizedBox(height: 10),
                        PatientCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: Text(l10n.notificationSettingsGreetingsTitle,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text)),
                                subtitle: Text(
                                    l10n.notificationSettingsGreetingsSubtitle,
                                    style: const TextStyle(
                                        fontSize: 12, color: PatientTokens.textSub)),
                                value: prefs.greetingNotificationsEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(prefs.copyWith(
                                        greetingNotificationsEnabled: value)))),
                        if (prefs.greetingNotificationsEnabled) ...[
                          const SizedBox(height: 10),
                          PatientCard(
                              child: SwitchListTile(
                                  contentPadding: EdgeInsets.zero,
                                  title: Text(l10n.notificationSettingsReligiousTitle,
                                      style: const TextStyle(
                                          fontWeight: FontWeight.w700,
                                          color: PatientTokens.text)),
                                  subtitle: Text(
                                      l10n.notificationSettingsReligiousSubtitle,
                                      style: const TextStyle(
                                          fontSize: 12, color: PatientTokens.textSub)),
                                  value: prefs.religiousGreetingsEnabled,
                                  onChanged: _saving
                                      ? null
                                      : (value) => _save(prefs.copyWith(
                                          religiousGreetingsEnabled: value)))),
                        ],
                        const SizedBox(height: 10),
                        PatientCard(
                            child: Column(children: [
                          SwitchListTile(
                              contentPadding: EdgeInsets.zero,
                              title: Text(l10n.notificationSettingsQuietHoursTitle,
                                  style: const TextStyle(
                                      fontWeight: FontWeight.w700,
                                      color: PatientTokens.text)),
                              subtitle: Text(
                                  l10n.notificationSettingsQuietHoursSubtitle,
                                  style: const TextStyle(
                                      fontSize: 12, color: PatientTokens.textSub)),
                              value: prefs.quietHoursEnabled,
                              onChanged: _saving
                                  ? null
                                  : (value) =>
                                      _save(prefs.copyWith(quietHoursEnabled: value))),
                          if (prefs.quietHoursEnabled) ...[
                            ListTile(
                                contentPadding: EdgeInsets.zero,
                                title: Text(l10n.notificationSettingsQuietHoursStart),
                                trailing: Text(prefs.quietHoursStartLocal,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text)),
                                onTap: _saving
                                    ? null
                                    : () => _pickTime(isStart: true, current: prefs)),
                            ListTile(
                                contentPadding: EdgeInsets.zero,
                                title: Text(l10n.notificationSettingsQuietHoursEnd),
                                trailing: Text(prefs.quietHoursEndLocal,
                                    style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        color: PatientTokens.text)),
                                onTap: _saving
                                    ? null
                                    : () => _pickTime(isStart: false, current: prefs)),
                          ]
                        ])),
                      ]));
  }
}

/// Inline (not a bottom-sheet-modal, unlike `language_modal.dart`'s profile
/// entry point) EN/SW toggle so the choice lives on the same page as every
/// other notification preference. Reuses `LocaleController` directly —
/// selecting a language here immediately retranslates the app, same as the
/// Profile screen's own language entry.
class _LanguageSection extends StatelessWidget {
  const _LanguageSection();

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final locale = context.watch<LocaleController>();
    return PatientCard(
        child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Text(l10n.languageTitle,
          style: const TextStyle(
              fontWeight: FontWeight.w700, color: PatientTokens.text)),
      const SizedBox(height: 10),
      Row(children: [
        Expanded(
            child: _LanguageChip(
                label: l10n.languageEnglish,
                selected: locale.locale.languageCode == 'en',
                onTap: () => locale.setLocale('en'))),
        const SizedBox(width: 8),
        Expanded(
            child: _LanguageChip(
                label: l10n.languageSwahili,
                selected: locale.locale.languageCode == 'sw',
                onTap: () => locale.setLocale('sw'))),
      ]),
    ]));
  }
}

class _LanguageChip extends StatelessWidget {
  const _LanguageChip(
      {required this.label, required this.selected, required this.onTap});
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
      child: Container(
          padding: const EdgeInsets.symmetric(vertical: 10),
          alignment: Alignment.center,
          decoration: BoxDecoration(
              color: selected ? PatientTokens.primaryLight : PatientTokens.surfaceSubtle,
              borderRadius: BorderRadius.circular(PatientTokens.cardRadius),
              border: Border.all(
                  color: selected ? PatientTokens.primary : PatientTokens.border)),
          child: Text(label,
              style: TextStyle(
                  fontWeight: FontWeight.w700,
                  color: selected ? PatientTokens.primary : PatientTokens.text))));
}
