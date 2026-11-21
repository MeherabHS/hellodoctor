import 'package:intl/intl.dart';

import '../../l10n/app_localizations.dart';
import '../l10n/kenya_time.dart';

/// "Today", "Yesterday" or "Thu 20 Nov", in Kenya time — the same day
/// labels the Peer Navigator chat uses. Used for when a consultation chat
/// stops accepting messages.
String consultationChatDayLabel(AppLocalizations l, DateTime instant) {
  final day = kenyaTime(instant);
  final today = kenyaTime(DateTime.now());
  final diff = DateTime(today.year, today.month, today.day)
      .difference(DateTime(day.year, day.month, day.day))
      .inDays;
  return switch (diff) {
    0 => l.peerToday,
    1 => l.peerYesterday,
    _ => DateFormat('EEE d MMM', l.localeName).format(day),
  };
}
