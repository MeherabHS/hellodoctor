import 'package:flutter/material.dart';
import '../../core/theme/tokens.dart';
import '../../core/utils/time_format.dart';
import 'doctor_models.dart';

class ClinicalMessageBubble extends StatelessWidget {
  const ClinicalMessageBubble(
      {super.key,
      required this.message,
      required this.myUserId,
      this.onViewAttachment});
  final MessageDto message;
  final String? myUserId;
  final VoidCallback? onViewAttachment;
  @override
  Widget build(BuildContext context) {
    final mine = myUserId != null && message.senderId == myUserId;
    final text = mine ? Colors.white : DoctorTokens.text;
    // Kenya display time; sender timestamps remain their authoritative UTC instants.
    final local = DateTime.tryParse(message.sentAt)
        ?.toUtc()
        .add(const Duration(hours: 3));
    final time = local == null ? '' : formatTime12h(local);
    return Align(
      alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
          key: ValueKey('message-${message.id}'),
          constraints:
              BoxConstraints(maxWidth: MediaQuery.sizeOf(context).width * .82),
          margin: const EdgeInsets.only(bottom: 12),
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
              color: mine ? DoctorTokens.primary : DoctorTokens.surface,
              border: mine ? null : Border.all(color: DoctorTokens.border),
              borderRadius: BorderRadius.circular(12)),
          child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (message.content.isNotEmpty)
                  Text(message.content,
                      style: TextStyle(color: text, height: 1.45)),
                if (message.attachmentId != null) ...[
                  Row(mainAxisSize: MainAxisSize.min, children: [
                    Icon(Icons.description_outlined, color: text, size: 20),
                    const SizedBox(width: 8),
                    Flexible(
                        child: Text('Clinical image',
                            style: TextStyle(
                                color: text, fontWeight: FontWeight.w600)))
                  ]),
                  if (message.attachmentSize != null)
                    Text(
                        '${(message.attachmentSize! / 1024).toStringAsFixed(1)} KiB',
                        style: TextStyle(color: text)),
                  TextButton(
                      onPressed: onViewAttachment,
                      style: TextButton.styleFrom(
                          foregroundColor: text,
                          minimumSize: const Size(48, 48)),
                      child: const Text('View image')),
                ],
                const SizedBox(height: 4),
                Text(time,
                    style: TextStyle(
                        fontSize: 11,
                        color: mine ? Colors.white70 : DoctorTokens.textSub)),
              ])),
    );
  }
}
