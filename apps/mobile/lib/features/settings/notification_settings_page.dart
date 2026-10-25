import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/theme/tokens.dart';
import '../../core/widgets/doctor_widgets.dart';
import '../doctor/doctor_pages.dart' show showMessage;
import '../notifications/notification_cubit.dart';
import '../notifications/notification_models.dart';

/// Milestone 3 task 11: a simple notification-settings screen using this
/// app's existing settings patterns (`SecuritySettingsPage`'s Scaffold/
/// AppBar shape, `DoctorCard` rows, `showTimePicker` the same way
/// `doctor_pages.dart`'s schedule-slot creation already does).
///
/// Milestone 4/5 added a `preferredLanguage` toggle below — CLAUDE.md scopes
/// the bilingual EN/SW *UI chrome* toggle to the Patient interface only, but
/// this is a narrower thing: which language the Doctor's own *notification
/// content* renders in (`NotificationService::render` already supports any
/// authenticated user, Doctor included — this was simply the first UI for
/// it). The screen itself stays English-only; no app-wide `AppLocalizations`
/// system is introduced here.
class NotificationSettingsPage extends StatefulWidget {
  const NotificationSettingsPage({super.key});
  @override
  State<NotificationSettingsPage> createState() =>
      _NotificationSettingsPageState();
}

class _NotificationSettingsPageState extends State<NotificationSettingsPage> {
  bool _loading = true;
  bool _saving = false;
  String? _error;
  NotificationPreferencesDto? _prefs;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
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
        _error = 'Unable to load notification settings.';
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
        language: next.preferredLanguage,
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
      showMessage(context, 'Could not save notification settings.');
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
    final prefs = _prefs;
    return Scaffold(
        backgroundColor: DoctorTokens.bg,
        appBar: AppBar(title: const Text('Notification settings')),
        body: _loading
            ? const Center(child: CircularProgressIndicator())
            : _error != null
                ? Center(
                    child: Column(mainAxisSize: MainAxisSize.min, children: [
                    Text(_error!, style: const TextStyle(color: DoctorTokens.textSub)),
                    const SizedBox(height: 12),
                    OutlinedButton(onPressed: _load, child: const Text('Retry')),
                  ]))
                : prefs == null
                    ? const SizedBox.shrink()
                    : ListView(padding: const EdgeInsets.all(16), children: [
                        DoctorCard(
                            child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                              const Text('Notification language',
                                  style: TextStyle(fontWeight: FontWeight.w700)),
                              const SizedBox(height: 10),
                              Row(children: [
                                Expanded(
                                    child: _LanguageChip(
                                        label: 'English',
                                        selected: prefs.preferredLanguage == 'en',
                                        onTap: _saving
                                            ? null
                                            : () => _save(
                                                prefs.copyWith(preferredLanguage: 'en')))),
                                const SizedBox(width: 8),
                                Expanded(
                                    child: _LanguageChip(
                                        label: 'Kiswahili',
                                        selected: prefs.preferredLanguage == 'sw',
                                        onTap: _saving
                                            ? null
                                            : () => _save(
                                                prefs.copyWith(preferredLanguage: 'sw')))),
                              ]),
                            ])),
                        const SizedBox(height: 10),
                        DoctorCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: const Text('Greetings',
                                    style: TextStyle(fontWeight: FontWeight.w700)),
                                subtitle: const Text(
                                    'Good-morning, birthday and Kenyan holiday messages',
                                    style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
                                value: prefs.greetingNotificationsEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(
                                        prefs.copyWith(greetingNotificationsEnabled: value)))),
                        if (prefs.greetingNotificationsEnabled) ...[
                          const SizedBox(height: 10),
                          DoctorCard(
                              child: SwitchListTile(
                                  contentPadding: EdgeInsets.zero,
                                  title: const Text('Religious holiday greetings',
                                      style: TextStyle(fontWeight: FontWeight.w700)),
                                  subtitle: const Text(
                                      'Eid, Good Friday, Easter and Christmas messages',
                                      style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
                                  value: prefs.religiousGreetingsEnabled,
                                  onChanged: _saving
                                      ? null
                                      : (value) => _save(
                                          prefs.copyWith(religiousGreetingsEnabled: value)))),
                        ],
                        const SizedBox(height: 10),
                        DoctorCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: const Text('Routine updates',
                                    style: TextStyle(fontWeight: FontWeight.w700)),
                                subtitle: const Text(
                                    'Article status and other operational notices',
                                    style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
                                value: prefs.operationalEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(
                                        prefs.copyWith(operationalEnabled: value)))),
                        const SizedBox(height: 10),
                        DoctorCard(
                            child: SwitchListTile(
                                contentPadding: EdgeInsets.zero,
                                title: const Text('Announcements',
                                    style: TextStyle(fontWeight: FontWeight.w700)),
                                subtitle: const Text(
                                    'Optional engagement and announcement notices',
                                    style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
                                value: prefs.engagementEnabled,
                                onChanged: _saving
                                    ? null
                                    : (value) => _save(
                                        prefs.copyWith(engagementEnabled: value)))),
                        const SizedBox(height: 10),
                        DoctorCard(
                            child: Column(children: [
                          SwitchListTile(
                              contentPadding: EdgeInsets.zero,
                              title: const Text('Quiet hours',
                                  style: TextStyle(fontWeight: FontWeight.w700)),
                              subtitle: const Text(
                                  'Hold non-urgent notifications during this window. '
                                  'Consultation reminders and time-sensitive alerts are never delayed.',
                                  style: TextStyle(fontSize: 12, color: DoctorTokens.textSub)),
                              value: prefs.quietHoursEnabled,
                              onChanged: _saving
                                  ? null
                                  : (value) =>
                                      _save(prefs.copyWith(quietHoursEnabled: value))),
                          if (prefs.quietHoursEnabled) ...[
                            ListTile(
                                contentPadding: EdgeInsets.zero,
                                title: const Text('Starts at'),
                                trailing: Text(prefs.quietHoursStartLocal,
                                    style: const TextStyle(fontWeight: FontWeight.w700)),
                                onTap: _saving
                                    ? null
                                    : () => _pickTime(isStart: true, current: prefs)),
                            ListTile(
                                contentPadding: EdgeInsets.zero,
                                title: const Text('Ends at'),
                                trailing: Text(prefs.quietHoursEndLocal,
                                    style: const TextStyle(fontWeight: FontWeight.w700)),
                                onTap: _saving
                                    ? null
                                    : () => _pickTime(isStart: false, current: prefs)),
                          ]
                        ])),
                      ]));
  }
}

class _LanguageChip extends StatelessWidget {
  const _LanguageChip(
      {required this.label, required this.selected, required this.onTap});
  final String label;
  final bool selected;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(12),
      child: Container(
          padding: const EdgeInsets.symmetric(vertical: 10),
          alignment: Alignment.center,
          decoration: BoxDecoration(
              color: selected
                  ? DoctorTokens.primaryDark.withValues(alpha: 0.12)
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(
                  color: selected ? DoctorTokens.primaryDark : DoctorTokens.textSub)),
          child: Text(label,
              style: TextStyle(
                  fontWeight: FontWeight.w700,
                  color: selected ? DoctorTokens.primaryDark : DoctorTokens.text))));
}
