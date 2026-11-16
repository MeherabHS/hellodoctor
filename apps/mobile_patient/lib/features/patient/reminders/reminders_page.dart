import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:intl/intl.dart';

import '../../../core/theme/tokens.dart';
import '../../../core/widgets/afya_widgets.dart';
import '../../../l10n/app_localizations.dart';
import '../modules/afya_module_widgets.dart';
import '../modules/patient_modules.dart';
import 'reminder_models.dart';
import 'reminder_services.dart';
import 'reminders_cubit.dart';

/// Icon, tint and localized copy for a reminder category.
class ReminderCategoryStyle {
  const ReminderCategoryStyle(this.icon, this.color, this.background);
  final IconData icon;
  final Color color, background;

  static ReminderCategoryStyle of(ReminderCategory c) => switch (c) {
        ReminderCategory.art => const ReminderCategoryStyle(
            Icons.medication_outlined,
            PatientTokens.tealCta,
            PatientTokens.infoCard),
        ReminderCategory.prep => const ReminderCategoryStyle(
            Icons.science_outlined,
            PatientTokens.tileGreen,
            PatientTokens.tileGreenBg),
        ReminderCategory.appointment => const ReminderCategoryStyle(
            Icons.event_available_outlined,
            PatientTokens.tileBlue,
            PatientTokens.tileBlueBg),
        ReminderCategory.testing => const ReminderCategoryStyle(
            Icons.colorize_outlined,
            PatientTokens.tilePurple,
            PatientTokens.tilePurpleBg),
        ReminderCategory.medication => const ReminderCategoryStyle(
            Icons.health_and_safety_outlined,
            PatientTokens.tileRed,
            PatientTokens.tileRedBg),
        ReminderCategory.other => const ReminderCategoryStyle(
            Icons.grid_view_rounded,
            PatientTokens.tileAmber,
            PatientTokens.tileAmberBg),
      };

  static String label(AppLocalizations l, ReminderCategory c) => switch (c) {
        ReminderCategory.art => l.reminderCatArt,
        ReminderCategory.prep => l.reminderCatPrep,
        ReminderCategory.appointment => l.reminderCatAppointment,
        ReminderCategory.testing => l.reminderCatTesting,
        ReminderCategory.medication => l.reminderCatMedication,
        ReminderCategory.other => l.reminderCatOther,
      };
}

String reminderRepeatLabel(AppLocalizations l, ReminderRepeat r) => switch (r) {
      ReminderRepeat.once => l.reminderRepeatOnce,
      ReminderRepeat.daily => l.reminderRepeatDaily,
      ReminderRepeat.weekly => l.reminderRepeatWeekly,
      ReminderRepeat.hourly => l.reminderRepeatHourly,
      ReminderRepeat.monthly => l.reminderRepeatMonthly,
    };

/// Short weekday name (1 = Monday) in the app's language.
String reminderWeekdayLabel(String locale, int weekday) =>
    DateFormat.E(locale).format(DateTime(2026, 11, 9 + weekday - 1));

/// Times of day as the locale writes them, joined with dots.
String reminderTimesLabel(String locale, List<int> minutesOfDay) => [
      for (final m in [...minutesOfDay]..sort())
        DateFormat.jm(locale).format(DateTime(2026, 1, 1, m ~/ 60, m % 60))
    ].join(' · ');

/// How a reminder repeats, as the card shows it: "Every 8 hours",
/// "Mon, Wed, Fri", "Weekdays", "Daily"...
String reminderRepeatSummary(AppLocalizations l, String locale, Reminder r) {
  switch (r.repeat) {
    case ReminderRepeat.hourly:
      return l.reminderEveryHours(r.everyHours);
    case ReminderRepeat.weekly:
      final days = r.ringDays;
      if (days.length == 7) return l.reminderEveryDay;
      if (days.join() == '12345') return l.reminderWeekdays;
      if (days.join() == '67') return l.reminderWeekends;
      return days.map((d) => reminderWeekdayLabel(locale, d)).join(', ');
    default:
      return reminderRepeatLabel(l, r.repeat);
  }
}

/// "Rings in 3 h 20 min", like a phone's clock app after setting an alarm.
String reminderRingsIn(AppLocalizations l, Duration d) {
  final minutes = (d.inSeconds / 60).ceil().clamp(1, 1 << 30);
  final String time;
  if (minutes < 60) {
    time = l.reminderDurationMinutes(minutes);
  } else if (minutes < 24 * 60) {
    time = l.reminderDurationHours(minutes ~/ 60, minutes % 60);
  } else {
    time = l.reminderDurationDays(minutes ~/ (24 * 60), (minutes ~/ 60) % 24);
  }
  return l.reminderRingsIn(time);
}

/// Reminders (reference panel 5): Upcoming / Past tabs, a card per reminder
/// and the privacy note. The reminders are the patient's own, stored on the
/// device; an empty list shows an empty state, never sample content.
class RemindersPage extends StatefulWidget {
  const RemindersPage(
      {super.key,
      required this.nav,
      required this.userId,
      this.store,
      this.scheduler});
  final PatientShellNav nav;
  final String userId;
  final ReminderStore? store;
  final ReminderScheduler? scheduler;
  @override
  State<RemindersPage> createState() => _RemindersPageState();
}

class _RemindersPageState extends State<RemindersPage>
    with WidgetsBindingObserver {
  RemindersCubit? _cubit;
  int _tab = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
  }

  /// Coming back from the system "Alarms & reminders" page: show the new
  /// state and re-arm the reminders with it.
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(_cubit?.refreshAlarmAccess());
    }
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _cubit ??= RemindersCubit(
        userId: widget.userId,
        store: widget.store ?? SecureReminderStore(),
        scheduler: widget.scheduler ?? LocalReminderScheduler(),
        text: reminderAlertText(AppLocalizations.of(context)))
      ..load();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _cubit?.close();
    super.dispose();
  }

  Future<void> _create() async {
    await Navigator.of(context).push(
        MaterialPageRoute(builder: (_) => CreateReminderPage(cubit: _cubit!)));
    if (mounted) setState(() => _tab = 0);
  }

  Future<void> _edit(Reminder r) async {
    await Navigator.of(context).push(MaterialPageRoute(
        builder: (_) => CreateReminderPage(cubit: _cubit!, existing: r)));
  }

  Future<void> _delete(Reminder r) async {
    final l = AppLocalizations.of(context);
    final ok = await showDialog<bool>(
        context: context,
        builder: (context) => AlertDialog(
              title: Text(l.reminderDeleteTitle),
              content: Text(r.title),
              actions: [
                TextButton(
                    onPressed: () => Navigator.pop(context, false),
                    child: Text(l.reminderCancel)),
                TextButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: Text(l.reminderDeleteConfirm,
                        style: const TextStyle(color: PatientTokens.danger))),
              ],
            ));
    if (ok == true) await _cubit!.remove(r.id);
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = Localizations.localeOf(context).toLanguageTag();
    return AfyaModuleScaffold(
      title: l.moduleReminders,
      nav: widget.nav,
      child: BlocBuilder<RemindersCubit, RemindersState>(
          bloc: _cubit,
          builder: (context, state) {
            final now = _cubit!.now;
            final list = _tab == 0 ? state.upcoming(now) : state.past(now);
            return ListView(
                padding: const EdgeInsets.fromLTRB(20, 6, 20, 24),
                children: [
                  AfyaSegmentedTabs(
                      expand: true,
                      labels: [l.remindersTabUpcoming, l.remindersTabPast],
                      selected: _tab,
                      onSelected: (i) => setState(() => _tab = i)),
                  const SizedBox(height: 16),
                  if (!state.notificationsAllowed) ...[
                    AfyaInfoCard(
                        icon: Icons.notifications_off_outlined,
                        title: l.moduleReminders,
                        body: l.remindersNotificationsOff,
                        background: PatientTokens.amberCard,
                        accent: PatientTokens.amberText),
                    const SizedBox(height: 14),
                  ] else if (!state.exactAlarmsAllowed &&
                      state.items.any((r) => r.enabled && !r.isPast(now))) ...[
                    AfyaInfoCard(
                        icon: Icons.alarm_off_rounded,
                        title: l.moduleReminders,
                        body: l.remindersExactAlarmsOff,
                        background: PatientTokens.amberCard,
                        accent: PatientTokens.amberText),
                    const SizedBox(height: 8),
                    Align(
                      alignment: Alignment.centerRight,
                      child: FilledButton.icon(
                          onPressed: _cubit!.allowExactAlarms,
                          style: FilledButton.styleFrom(
                              backgroundColor: PatientTokens.amberText),
                          icon: const Icon(Icons.alarm_on_rounded, size: 18),
                          label: Text(l.remindersExactAlarmsAllow)),
                    ),
                    const SizedBox(height: 14),
                  ],
                  if (state.loading)
                    const Padding(
                        padding: EdgeInsets.all(32),
                        child: Center(child: CircularProgressIndicator()))
                  else if (list.isEmpty)
                    AfyaUnavailableNote(
                        icon: Icons.notifications_none_rounded,
                        title: _tab == 0
                            ? l.remindersEmptyUpcomingTitle
                            : l.remindersEmptyPastTitle,
                        body: _tab == 0
                            ? l.remindersEmptyUpcomingBody
                            : l.remindersEmptyPastBody)
                  else
                    for (final r in list)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 14),
                        child: _ReminderCard(
                            reminder: r,
                            now: now,
                            when: r.nextAlert(now) ?? r.at,
                            locale: locale,
                            onEdit: () => _edit(r),
                            onToggle: (on) => _cubit!.setEnabled(r.id, on),
                            onDelete: () => _delete(r)),
                      ),
                  const SizedBox(height: 4),
                  AfyaPillButton(
                      label: l.remindersAdd,
                      icon: Icons.add_alert_rounded,
                      onPressed: _create),
                  const SizedBox(height: 18),
                  AfyaInfoCard(
                      icon: Icons.lock_rounded,
                      title: l.remindersPrivacy1,
                      body: '${l.remindersPrivacy2}\n${l.remindersDeviceNote}'),
                ]);
          }),
    );
  }
}

class _ReminderCard extends StatelessWidget {
  const _ReminderCard(
      {required this.reminder,
      required this.now,
      required this.when,
      required this.locale,
      required this.onEdit,
      required this.onToggle,
      required this.onDelete});
  final Reminder reminder;
  final DateTime now, when;
  final String locale;
  final VoidCallback onEdit;
  final ValueChanged<bool> onToggle;
  final VoidCallback onDelete;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final style = ReminderCategoryStyle.of(reminder.category);
    final on = reminder.enabled;
    final ink = on ? PatientTokens.navy : PatientTokens.navInactive;
    final date = DateFormat('EEE, d MMM', locale).format(when);
    final time = DateFormat.jm(locale).format(when);
    final upcoming = when.isAfter(now);
    return AfyaCard(
      onTap: onEdit,
      padding: const EdgeInsets.fromLTRB(12, 14, 4, 12),
      child: Row(crossAxisAlignment: CrossAxisAlignment.start, children: [
        AfyaIconTile(
            icon: style.icon,
            color: on ? style.color : PatientTokens.navInactive,
            background: style.background,
            size: 46),
        const SizedBox(width: 12),
        Expanded(
          child:
              Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            // The next ring time, large, as a clock app shows an alarm.
            Text(time,
                style: TextStyle(
                    fontSize: 26,
                    height: 1.1,
                    fontWeight: FontWeight.w800,
                    letterSpacing: -.5,
                    color: ink)),
            const SizedBox(height: 4),
            Text(reminder.title,
                style: TextStyle(
                    fontSize: 14, fontWeight: FontWeight.w700, color: ink)),
            if (reminder.note.isNotEmpty) ...[
              const SizedBox(height: 3),
              Text(reminder.note,
                  style: const TextStyle(
                      fontSize: 12.5, height: 1.4, color: PatientTokens.ink)),
            ],
            const SizedBox(height: 8),
            Wrap(
                spacing: 6,
                runSpacing: 6,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: [
                  _Tag(
                      icon: reminder.repeat == ReminderRepeat.once
                          ? Icons.event_outlined
                          : Icons.repeat_rounded,
                      text: reminder.repeat == ReminderRepeat.once
                          ? date
                          : reminderRepeatSummary(l, locale, reminder)),
                  if (on && upcoming)
                    _Tag(
                        icon: Icons.alarm_rounded,
                        text: reminderRingsIn(l, when.difference(now)),
                        highlight: true),
                ]),
          ]),
        ),
        Column(mainAxisSize: MainAxisSize.min, children: [
          Semantics(
            label: on ? l.reminderAlarmOn : l.reminderAlarmOff,
            child: Switch(value: on, onChanged: onToggle),
          ),
          IconButton(
              onPressed: onDelete,
              tooltip: l.reminderDeleteConfirm,
              icon: const Icon(Icons.delete_outline_rounded,
                  color: PatientTokens.navInactive)),
        ]),
      ]),
    );
  }
}

class _Tag extends StatelessWidget {
  const _Tag({required this.icon, required this.text, this.highlight = false});
  final IconData icon;
  final String text;
  final bool highlight;
  @override
  Widget build(BuildContext context) {
    final color = highlight ? PatientTokens.navActive : PatientTokens.ink;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
          color: highlight ? const Color(0xFFE3F6EC) : const Color(0xFFF1F4F3),
          borderRadius: BorderRadius.circular(20)),
      child: Row(mainAxisSize: MainAxisSize.min, children: [
        Icon(icon, size: 14, color: color),
        const SizedBox(width: 4),
        Flexible(
          child: Text(text,
              style: TextStyle(
                  fontSize: 12, fontWeight: FontWeight.w600, color: color)),
        ),
      ]),
    );
  }
}

/// Create-reminder form (product owner's "Reminder Creation Suite" design).
///
/// One screen serves all six categories: choosing a category changes the
/// heading, description, header icon, suggested title and note hint, as the
/// six panels of the design show. The layout scrolls and does not depend on
/// screen size.
class CreateReminderPage extends StatefulWidget {
  const CreateReminderPage(
      {super.key,
      required this.cubit,
      this.initialCategory = ReminderCategory.art,
      this.existing});
  final RemindersCubit cubit;
  final ReminderCategory initialCategory;

  /// When set, the form edits this reminder instead of creating one.
  final Reminder? existing;
  @override
  State<CreateReminderPage> createState() => _CreateReminderPageState();
}

class _CreateReminderPageState extends State<CreateReminderPage> {
  late ReminderCategory _category =
      widget.existing?.category ?? widget.initialCategory;
  late ReminderRepeat _repeat =
      widget.existing?.repeat ?? _defaultRepeat(_category);
  late DateTime _at = widget.existing?.at ?? _defaultTime();

  /// Chosen ring days for "Selected days" (1 = Monday). Starts from the
  /// saved days, or the start date's weekday.
  late Set<int> _weekdays = {
    ...(widget.existing?.ringDays ?? [_at.weekday])
  };
  late int _everyHours = widget.existing?.everyHours ?? 8;
  late final _title = TextEditingController(text: widget.existing?.title);
  late final _note = TextEditingController(text: widget.existing?.note);

  /// True until the patient types their own title; while true, the title
  /// follows the selected category's suggestion. An edited reminder keeps
  /// its own title.
  late bool _titleIsSuggestion = widget.existing == null;
  bool _saving = false;
  String? _error;

  DateTime _defaultTime() {
    final now = widget.cubit.now;
    final next = DateTime(now.year, now.month, now.day, 8);
    return next.isAfter(now) ? next : next.add(const Duration(days: 1));
  }

  ReminderRepeat _defaultRepeat(ReminderCategory c) => switch (c) {
        ReminderCategory.art ||
        ReminderCategory.prep ||
        ReminderCategory.medication =>
          ReminderRepeat.daily,
        _ => ReminderRepeat.once,
      };

  String _suggestion(AppLocalizations l) => switch (_category) {
        ReminderCategory.art => l.reminderDefaultArt,
        ReminderCategory.prep => l.reminderDefaultPrep,
        ReminderCategory.appointment => l.reminderDefaultAppointment,
        ReminderCategory.testing => l.reminderDefaultTesting,
        ReminderCategory.medication => l.reminderDefaultMedication,
        ReminderCategory.other => l.reminderDefaultOther,
      };

  String _description(AppLocalizations l) => switch (_category) {
        ReminderCategory.art => l.reminderDescArt,
        ReminderCategory.prep => l.reminderDescPrep,
        ReminderCategory.appointment => l.reminderDescAppointment,
        ReminderCategory.testing => l.reminderDescTesting,
        ReminderCategory.medication => l.reminderDescMedication,
        ReminderCategory.other => l.reminderDescOther,
      };

  String _noteHint(AppLocalizations l) => switch (_category) {
        ReminderCategory.art => l.reminderNoteHintArt,
        ReminderCategory.prep => l.reminderNoteHintPrep,
        ReminderCategory.appointment => l.reminderNoteHintAppointment,
        ReminderCategory.testing => l.reminderNoteHintTesting,
        ReminderCategory.medication => l.reminderNoteHintMedication,
        ReminderCategory.other => l.reminderNoteHintOther,
      };

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_titleIsSuggestion) {
      _title.text = _suggestion(AppLocalizations.of(context));
    }
  }

  @override
  void dispose() {
    _title.dispose();
    _note.dispose();
    super.dispose();
  }

  void _selectCategory(ReminderCategory c) => setState(() {
        _category = c;
        _repeat = _defaultRepeat(c);
        if (_titleIsSuggestion) {
          _title.text = _suggestion(AppLocalizations.of(context));
        }
      });

  Future<void> _pickDate() async {
    final now = widget.cubit.now;
    final picked = await showDatePicker(
        context: context,
        initialDate: _at,
        firstDate: DateTime(now.year, now.month, now.day),
        lastDate: DateTime(now.year + 5));
    if (picked != null) {
      setState(() => _at = DateTime(
          picked.year, picked.month, picked.day, _at.hour, _at.minute));
    }
  }

  Future<void> _pickTime() async {
    final picked = await showTimePicker(
        context: context, initialTime: TimeOfDay.fromDateTime(_at));
    if (picked != null) {
      setState(() => _at =
          DateTime(_at.year, _at.month, _at.day, picked.hour, picked.minute));
    }
  }

  Future<void> _save() async {
    final l = AppLocalizations.of(context);
    final title = _title.text.trim();
    if (title.isEmpty) return setState(() => _error = l.reminderTitleRequired);
    // A repeating reminder may start at a time already gone today; it will
    // alert at its next occurrence. A one-time reminder must be in the future.
    if (_repeat == ReminderRepeat.once && !_at.isAfter(widget.cubit.now)) {
      return setState(() => _error = l.reminderTimePast);
    }
    if (_repeat == ReminderRepeat.weekly && _weekdays.isEmpty) {
      return setState(() => _error = l.reminderPickDay);
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    final existing = widget.existing;
    final ok = existing == null
        ? await widget.cubit.add(Reminder(
            id: '${widget.cubit.now.microsecondsSinceEpoch}',
            category: _category,
            title: title,
            note: _note.text.trim(),
            at: _at,
            repeat: _repeat,
            weekdays: _repeat == ReminderRepeat.weekly ? _weekdays : const {},
            everyHours: _everyHours))
        : await widget.cubit.update(existing.copyWith(
            category: _category,
            title: title,
            note: _note.text.trim(),
            at: _at,
            repeat: _repeat,
            weekdays: _repeat == ReminderRepeat.weekly ? _weekdays : const {},
            everyHours: _everyHours,
            enabled: true));
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() {
        _saving = false;
        _error = l.reminderSaveFailed;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    final locale = Localizations.localeOf(context).toLanguageTag();
    final style = ReminderCategoryStyle.of(_category);
    final name = ReminderCategoryStyle.label(l, _category);
    return Scaffold(
      backgroundColor: const Color(0xFFDDF3E6),
      body: DecoratedBox(
        decoration: const BoxDecoration(
            gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: [Color(0xFFEAF8F0), Color(0xFFCDEEDB)])),
        child: SafeArea(
          child: Center(
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 520),
              child: ListView(
                  padding: const EdgeInsets.fromLTRB(16, 4, 16, 20),
                  children: [
                    Row(children: [
                      IconButton(
                          onPressed: () => Navigator.maybePop(context),
                          tooltip: MaterialLocalizations.of(context)
                              .backButtonTooltip,
                          icon: const Icon(Icons.arrow_back_rounded,
                              color: PatientTokens.navActive)),
                      const Expanded(
                          child: Center(child: AfyaBrandLockup(markSize: 34))),
                      const SizedBox(width: 48),
                    ]),
                    const SizedBox(height: 6),
                    _Header(
                        title: l.reminderCreateTitle(name),
                        description: _description(l),
                        icon: style.icon),
                    const SizedBox(height: 10),
                    Container(
                      padding: const EdgeInsets.fromLTRB(16, 16, 16, 18),
                      decoration: BoxDecoration(
                          color: Colors.white.withValues(alpha: .62),
                          borderRadius: BorderRadius.circular(24),
                          border: Border.all(
                              color: Colors.white.withValues(alpha: .9))),
                      child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _FieldLabel(
                                Icons.edit_outlined, l.reminderTitleLabel),
                            TextField(
                                controller: _title,
                                maxLength: 80,
                                onChanged: (_) => _titleIsSuggestion = false,
                                decoration: _input(null)),
                            const SizedBox(height: 14),
                            _FieldLabel(
                                Icons.layers_outlined, l.reminderCategoryLabel),
                            _CategoryGrid(
                                selected: _category,
                                onSelected: _selectCategory),
                            const SizedBox(height: 16),
                            _FieldLabel(
                                Icons.calendar_month_outlined,
                                _repeat == ReminderRepeat.once
                                    ? l.reminderDateTimeLabel
                                    : '${l.reminderDateTimeLabel} · '
                                        '${l.reminderStartsLabel}'),
                            Container(
                              decoration: BoxDecoration(
                                  color: Colors.white.withValues(alpha: .8),
                                  borderRadius: BorderRadius.circular(16)),
                              child: IntrinsicHeight(
                                child: Row(children: [
                                  Expanded(
                                    flex: 3,
                                    child: _PickerButton(
                                        icon: Icons.event_outlined,
                                        text: DateFormat('EEE, d MMM y', locale)
                                            .format(_at),
                                        onTap: _pickDate),
                                  ),
                                  const VerticalDivider(
                                      width: 1, indent: 10, endIndent: 10),
                                  Expanded(
                                    flex: 2,
                                    child: _PickerButton(
                                        icon: Icons.schedule_rounded,
                                        text: DateFormat.jm(locale).format(_at),
                                        onTap: _pickTime),
                                  ),
                                ]),
                              ),
                            ),
                            const SizedBox(height: 16),
                            _FieldLabel(
                                Icons.repeat_rounded, l.reminderRepeatLabel),
                            _RepeatPicker(
                                selected: _repeat,
                                onSelected: (r) => setState(() {
                                      _repeat = r;
                                      _error = null;
                                    })),
                            if (_repeat == ReminderRepeat.weekly) ...[
                              const SizedBox(height: 14),
                              _FieldLabel(Icons.event_repeat_rounded,
                                  l.reminderDaysLabel),
                              _WeekdayPicker(
                                  locale: locale,
                                  selected: _weekdays,
                                  onChanged: (days) => setState(() {
                                        _weekdays = days;
                                        _error = null;
                                      })),
                            ],
                            if (_repeat == ReminderRepeat.hourly) ...[
                              const SizedBox(height: 14),
                              _FieldLabel(Icons.timelapse_rounded,
                                  l.reminderIntervalLabel),
                              _HourIntervalPicker(
                                  selected: _everyHours,
                                  onSelected: (h) =>
                                      setState(() => _everyHours = h)),
                              const SizedBox(height: 8),
                              Text(
                                  l.reminderRingTimes(reminderTimesLabel(
                                      locale,
                                      Reminder(
                                              id: '',
                                              category: _category,
                                              title: '',
                                              at: _at,
                                              repeat: ReminderRepeat.hourly,
                                              everyHours: _everyHours)
                                          .ringMinutesOfDay)),
                                  style: const TextStyle(
                                      fontSize: 12.5,
                                      height: 1.4,
                                      fontWeight: FontWeight.w600,
                                      color: Color(0xFF0B3D33))),
                            ],
                            const SizedBox(height: 16),
                            _FieldLabel(Icons.sticky_note_2_outlined,
                                l.reminderNoteLabel),
                            TextField(
                                controller: _note,
                                maxLength: 140,
                                maxLines: 2,
                                minLines: 1,
                                decoration: _input(_noteHint(l))),
                            if (_error != null)
                              Padding(
                                padding: const EdgeInsets.only(top: 6),
                                child: Text(_error!,
                                    style: const TextStyle(
                                        fontSize: 13,
                                        fontWeight: FontWeight.w600,
                                        color: PatientTokens.danger)),
                              ),
                            const SizedBox(height: 14),
                            AfyaPillButton(
                                label: l.reminderSave,
                                icon: Icons.notifications_none_rounded,
                                color: PatientTokens.navActive,
                                onPressed: _saving ? null : _save),
                          ]),
                    ),
                  ]),
            ),
          ),
        ),
      ),
    );
  }

  InputDecoration _input(String? hint) => InputDecoration(
      hintText: hint,
      counterText: '',
      isDense: true,
      fillColor: Colors.white.withValues(alpha: .8),
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 13),
      enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(16), borderSide: BorderSide.none),
      border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(16), borderSide: BorderSide.none),
      focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(16),
          borderSide:
              const BorderSide(color: PatientTokens.primary, width: 1.5)));
}

/// Heading and description on the left; the illustration with the selected
/// category's icon on the right.
class _Header extends StatelessWidget {
  const _Header(
      {required this.title, required this.description, required this.icon});
  final String title, description;
  final IconData icon;
  @override
  Widget build(BuildContext context) =>
      Row(crossAxisAlignment: CrossAxisAlignment.center, children: [
        Expanded(
          flex: 11,
          child: Padding(
            padding: const EdgeInsets.only(left: 6),
            child:
                Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
              Text(title,
                  style: const TextStyle(
                      fontSize: 23,
                      height: 1.15,
                      fontWeight: FontWeight.w800,
                      letterSpacing: -.3,
                      color: Color(0xFF0B3D33))),
              const SizedBox(height: 6),
              Text(description,
                  style: const TextStyle(
                      fontSize: 13, height: 1.4, color: Color(0xFF0B3D33))),
            ]),
          ),
        ),
        Expanded(
          flex: 10,
          child: Stack(alignment: Alignment.center, children: [
            Image.asset('assets/illustrations/woman_phone_bell.webp',
                height: 138, fit: BoxFit.contain, excludeFromSemantics: true),
            Positioned(
              right: 0,
              top: 6,
              child: Container(
                  width: 46,
                  height: 46,
                  decoration: const BoxDecoration(
                      color: Colors.white, shape: BoxShape.circle),
                  child: Icon(icon, size: 26, color: PatientTokens.navActive)),
            ),
          ]),
        ),
      ]);
}

class _FieldLabel extends StatelessWidget {
  const _FieldLabel(this.icon, this.text);
  final IconData icon;
  final String text;
  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(left: 2, bottom: 8),
        child: Row(children: [
          Icon(icon, size: 19, color: const Color(0xFF0B3D33)),
          const SizedBox(width: 9),
          Flexible(
            child: Text(text,
                style: const TextStyle(
                    fontSize: 14,
                    fontWeight: FontWeight.w700,
                    color: Color(0xFF0B3D33))),
          ),
        ]),
      );
}

class _PickerButton extends StatelessWidget {
  const _PickerButton(
      {required this.icon, required this.text, required this.onTap});
  final IconData icon;
  final String text;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(16),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 13),
          child: Row(children: [
            Icon(icon, size: 20, color: const Color(0xFF0B3D33)),
            const SizedBox(width: 8),
            Flexible(
              child: Text(text,
                  style: const TextStyle(
                      fontSize: 13.5,
                      fontWeight: FontWeight.w600,
                      color: Color(0xFF0B3D33))),
            ),
          ]),
        ),
      );
}

/// Six category chips in a 3 x 2 grid; the selected one is outlined in green
/// with a check badge.
class _CategoryGrid extends StatelessWidget {
  const _CategoryGrid({required this.selected, required this.onSelected});
  final ReminderCategory selected;
  final ValueChanged<ReminderCategory> onSelected;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    Widget chip(ReminderCategory c) {
      final on = c == selected;
      return Semantics(
        button: true,
        selected: on,
        child: Stack(clipBehavior: Clip.none, children: [
          Material(
            color: on
                ? const Color(0xFFE3F6EC)
                : Colors.white.withValues(alpha: .8),
            shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(16),
                side: BorderSide(
                    color: on ? PatientTokens.primary : Colors.transparent,
                    width: 1.5)),
            child: InkWell(
              onTap: () => onSelected(c),
              borderRadius: BorderRadius.circular(16),
              child: Container(
                constraints: const BoxConstraints(minHeight: 60),
                alignment: Alignment.center,
                padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 8),
                child: Column(mainAxisSize: MainAxisSize.min, children: [
                  Icon(ReminderCategoryStyle.of(c).icon,
                      size: 24, color: PatientTokens.navActive),
                  const SizedBox(height: 4),
                  Text(ReminderCategoryStyle.label(l, c),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w700,
                          color: Color(0xFF0B3D33))),
                ]),
              ),
            ),
          ),
          if (on)
            const Positioned(
                right: -4,
                top: -4,
                child: Icon(Icons.check_circle_rounded,
                    size: 20, color: PatientTokens.navActive)),
        ]),
      );
    }

    const cats = ReminderCategory.values;
    return Column(children: [
      for (var row = 0; row < 2; row++)
        Padding(
          padding: EdgeInsets.only(bottom: row == 0 ? 10 : 0),
          child: Row(children: [
            for (var col = 0; col < 3; col++) ...[
              if (col > 0) const SizedBox(width: 10),
              Expanded(child: chip(cats[row * 3 + col])),
            ]
          ]),
        ),
    ]);
  }
}

/// Repeat choices as selectable chips, like a clock app's repeat row.
class _RepeatPicker extends StatelessWidget {
  const _RepeatPicker({required this.selected, required this.onSelected});
  final ReminderRepeat selected;
  final ValueChanged<ReminderRepeat> onSelected;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Wrap(spacing: 8, runSpacing: 8, children: [
      for (final r in const [
        ReminderRepeat.once,
        ReminderRepeat.daily,
        ReminderRepeat.weekly,
        ReminderRepeat.hourly,
        ReminderRepeat.monthly,
      ])
        _Choice(
            label: reminderRepeatLabel(l, r),
            selected: r == selected,
            onTap: () => onSelected(r)),
    ]);
  }
}

/// Seven day toggles plus Every day / Weekdays / Weekends shortcuts.
class _WeekdayPicker extends StatelessWidget {
  const _WeekdayPicker(
      {required this.locale, required this.selected, required this.onChanged});
  final String locale;
  final Set<int> selected;
  final ValueChanged<Set<int>> onChanged;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    bool isSet(Set<int> days) =>
        selected.length == days.length && selected.containsAll(days);
    const all = {1, 2, 3, 4, 5, 6, 7}, work = {1, 2, 3, 4, 5}, rest = {6, 7};
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      Wrap(spacing: 6, runSpacing: 6, children: [
        for (var d = 1; d <= 7; d++)
          _Choice(
              label: reminderWeekdayLabel(locale, d),
              selected: selected.contains(d),
              compact: true,
              onTap: () => onChanged(selected.contains(d)
                  ? ({...selected}..remove(d))
                  : {...selected, d})),
      ]),
      const SizedBox(height: 8),
      Wrap(spacing: 8, runSpacing: 8, children: [
        _Choice(
            label: l.reminderEveryDay,
            selected: isSet(all),
            outline: true,
            onTap: () => onChanged({...all})),
        _Choice(
            label: l.reminderWeekdays,
            selected: isSet(work),
            outline: true,
            onTap: () => onChanged({...work})),
        _Choice(
            label: l.reminderWeekends,
            selected: isSet(rest),
            outline: true,
            onTap: () => onChanged({...rest})),
      ]),
    ]);
  }
}

/// "Every 1, 2, 3, 4, 6, 8 or 12 hours", counted from the start time.
class _HourIntervalPicker extends StatelessWidget {
  const _HourIntervalPicker({required this.selected, required this.onSelected});
  final int selected;
  final ValueChanged<int> onSelected;
  @override
  Widget build(BuildContext context) {
    final l = AppLocalizations.of(context);
    return Wrap(spacing: 8, runSpacing: 8, children: [
      for (final h in reminderHourIntervals)
        _Choice(
            label: l.reminderEveryHours(h),
            selected: h == selected,
            onTap: () => onSelected(h)),
    ]);
  }
}

class _Choice extends StatelessWidget {
  const _Choice(
      {required this.label,
      required this.selected,
      required this.onTap,
      this.compact = false,
      this.outline = false});
  final String label;
  final bool selected, compact, outline;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) {
    final filled = selected && !outline;
    return Semantics(
      button: true,
      selected: selected,
      child: Material(
        color: filled
            ? PatientTokens.navActive
            : selected
                ? const Color(0xFFE3F6EC)
                : Colors.white.withValues(alpha: .8),
        shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(20),
            side: BorderSide(
                color: selected ? PatientTokens.navActive : Colors.transparent,
                width: 1.2)),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(20),
          child: Container(
            constraints:
                BoxConstraints(minWidth: compact ? 44 : 0, minHeight: 38),
            padding: EdgeInsets.symmetric(horizontal: compact ? 8 : 14),
            // Sized to the label: an aligned Container would stretch to the
            // whole row inside the Wrap.
            child: Center(
              widthFactor: 1,
              heightFactor: 1,
              child: Text(label,
                  style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w700,
                      color: filled ? Colors.white : const Color(0xFF0B3D33))),
            ),
          ),
        ),
      ),
    );
  }
}

/// Localized alarm channel name and action labels.
ReminderAlertText reminderAlertText(AppLocalizations l) => ReminderAlertText(
    channelName: l.reminderChannelName,
    doneLabel: l.reminderAlarmDone,
    snoozeLabel: l.reminderAlarmSnooze);
