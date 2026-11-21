import 'package:flutter/material.dart';

import '../../core/network/api_client.dart';

/// Opens the consultation's chat over a live call (product owner,
/// 2026-11-20: chat and video at the same time, as on Doctor Web). The call
/// screen stays mounted underneath, so audio and video keep running while the
/// sheet is open; closing the sheet returns to the full call view.
Future<void> showInCallChat(
    BuildContext context, Future<Widget> Function() loadChat) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    backgroundColor: Colors.transparent,
    barrierColor: Colors.black26,
    builder: (_) => FractionallySizedBox(
      heightFactor: 0.82,
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(22)),
        child: _InCallChat(loadChat: loadChat),
      ),
    ),
  );
}

class _InCallChat extends StatefulWidget {
  const _InCallChat({required this.loadChat});
  final Future<Widget> Function() loadChat;
  @override
  State<_InCallChat> createState() => _InCallChatState();
}

class _InCallChatState extends State<_InCallChat> {
  late Future<Widget> _chat = widget.loadChat();

  @override
  Widget build(BuildContext context) => ColoredBox(
        color: Colors.white,
        child: FutureBuilder<Widget>(
          future: _chat,
          builder: (context, snapshot) {
            if (snapshot.hasData) return snapshot.data!;
            if (snapshot.hasError) {
              final error = snapshot.error;
              return Center(
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(mainAxisSize: MainAxisSize.min, children: [
                    const Icon(Icons.chat_bubble_outline_rounded, size: 36),
                    const SizedBox(height: 12),
                    Text(
                      error is ApiException
                          ? error.userMessage
                          : 'The chat could not be opened.',
                      textAlign: TextAlign.center,
                    ),
                    const SizedBox(height: 12),
                    TextButton(
                      onPressed: () =>
                          setState(() => _chat = widget.loadChat()),
                      child: const Text('Retry'),
                    ),
                  ]),
                ),
              );
            }
            return const Center(child: CircularProgressIndicator());
          },
        ),
      );
}
