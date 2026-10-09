import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import '../../core/network/api_client.dart';
import '../../core/theme/tokens.dart';
import 'doctor_cubit.dart';

/// Boundary for the future Agora SDK adapter. The server contract is exercised
/// here, but this client deliberately does not render fake media or claim a
/// call is active while Agora credentials/provider integration is deferred.
class AgoraSession {
  const AgoraSession({required this.channel, required this.appId});
  final String channel;
  final String appId;
}

class AgoraPage extends StatefulWidget {
  const AgoraPage({super.key, required this.appointmentId});
  final String appointmentId;
  @override
  State<AgoraPage> createState() => _AgoraState();
}

class _AgoraState extends State<AgoraPage> {
  bool loading = true;
  String? error;
  AgoraSession? session;

  @override
  void initState() {
    super.initState();
    authorize();
  }

  Future<void> authorize() async {
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final data =
          await context.read<DoctorCubit>().repo.rtcToken(widget.appointmentId);
      final channel = data['channel_name']?.toString();
      final appId = data['app_id']?.toString();
      if (channel == null ||
          appId == null ||
          channel.isEmpty ||
          appId.isEmpty) {
        throw const ApiException(
            422, 'The RTC authorization response is incomplete.');
      }
      session = AgoraSession(channel: channel, appId: appId);
    } on ApiException catch (e) {
      error = e.userMessage;
    } catch (_) {
      error = 'The consultation call could not be authorized.';
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Consultation call')),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: loading
              ? const CircularProgressIndicator()
              : error != null
                  ? Column(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(Icons.cloud_off,
                          size: 42, color: DoctorTokens.textSub),
                      const SizedBox(height: 12),
                      Text(error!, textAlign: TextAlign.center),
                      TextButton(
                          onPressed: authorize, child: const Text('Retry')),
                    ])
                  : Column(mainAxisSize: MainAxisSize.min, children: [
                      const Icon(Icons.videocam_off_outlined,
                          size: 52, color: DoctorTokens.primary),
                      const SizedBox(height: 14),
                      const Text('Live video is temporarily unavailable',
                          style: TextStyle(
                              fontSize: 18, fontWeight: FontWeight.w800),
                          textAlign: TextAlign.center),
                      const SizedBox(height: 8),
                      const Text(
                          'This consultation is authorized and ready for the Agora adapter. No remote video or fake call state is shown while Agora integration is deferred.',
                          textAlign: TextAlign.center),
                      const SizedBox(height: 16),
                      Text('Channel prepared: ${session!.channel}',
                          style: const TextStyle(color: DoctorTokens.textSub)),
                    ]),
        ),
      ),
    );
  }
}
