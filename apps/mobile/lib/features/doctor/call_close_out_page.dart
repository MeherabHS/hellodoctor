import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';

import '../../core/theme/tokens.dart';
import '../../core/widgets/doctor_widgets.dart';
import 'agora_page.dart';
import 'doctor_cubit.dart';
import 'doctor_models.dart';
import 'doctor_pages.dart';

/// Required close-out after the Doctor ends a video call (product-owner
/// rule, 2026-11-13). It cannot be dismissed: the Doctor dispatches the Rx,
/// records why no Rx is needed, refers/escalates, or rejoins the call.
/// Every decision is recorded by the server, never by this screen alone.
class CallCloseOutPage extends StatefulWidget {
  const CallCloseOutPage({super.key, required this.appointment});
  final AppointmentDto appointment;

  /// The appointment from the loaded queue, or a bare one with just the id.
  static AppointmentDto resolve(BuildContext context, String appointmentId) {
    for (final a in context.read<DoctorCubit>().state.queue) {
      if (a is AppointmentDto && a.id == appointmentId) return a;
    }
    return AppointmentDto(
        id: appointmentId, status: 'IN_CONSULTATION', lifecycle: '');
  }

  @override
  State<CallCloseOutPage> createState() => _CallCloseOutPageState();
}

class _CallCloseOutPageState extends State<CallCloseOutPage> {
  bool busy = false;

  AppointmentDto get a => widget.appointment;

  void _closed(String message) {
    showMessage(context, message);
    Navigator.pop(context);
  }

  Future<void> _dispatchRx() async {
    final sent = await Navigator.push<bool>(
        context,
        MaterialPageRoute(
            builder: (_) =>
                ScannerPage(initialAppointment: a, closeOnDispatch: true)));
    if (sent == true && mounted) Navigator.pop(context);
  }

  Future<void> _record(Future<void> Function() call, String done) async {
    if (busy) return;
    setState(() => busy = true);
    final cubit = context.read<DoctorCubit>();
    try {
      await call();
      await cubit.loadAll();
      if (mounted) _closed(done);
    } catch (e) {
      if (mounted) {
        showError(context, e,
            conflict: 'A decision is already recorded for this consultation.');
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> _noRx() async {
    final reason = await askNoRxReason(context);
    if (reason == null || !mounted) return;
    final repo = context.read<DoctorCubit>().repo;
    await _record(() => repo.completeNoRx(a.id, reason),
        'Consultation completed — no prescription needed.');
  }

  void _rejoin() => Navigator.pushReplacement(
      context,
      MaterialPageRoute(
          builder: (_) =>
              AgoraPage(appointmentId: a.id, closeOutOnExit: true)));

  @override
  Widget build(BuildContext context) {
    return PopScope(
        canPop: false,
        onPopInvokedWithResult: (didPop, _) {
          if (!didPop) {
            showMessage(context,
                'Record the prescription decision before leaving this consultation.');
          }
        },
        child: Scaffold(
            backgroundColor: DoctorTokens.bg,
            appBar: AppBar(
                automaticallyImplyLeading: false,
                title: const Text('Close consultation')),
            body: ListView(padding: const EdgeInsets.all(18), children: [
              DoctorCard(
                  child: Column(children: [
                const CircleAvatar(
                    radius: 30,
                    backgroundColor: DoctorTokens.noticeBg,
                    child: Icon(Icons.call_end,
                        size: 30, color: DoctorTokens.primaryDark)),
                const SizedBox(height: 12),
                const Text('Video call ended',
                    style: TextStyle(
                        fontSize: 19,
                        fontWeight: FontWeight.w800,
                        color: DoctorTokens.navy)),
                if (a.patientName != null) ...[
                  const SizedBox(height: 4),
                  Text(a.patientName!,
                      style: const TextStyle(
                          fontSize: 15,
                          fontWeight: FontWeight.w700,
                          color: DoctorTokens.slate)),
                ],
                const SizedBox(height: 10),
                const Text(
                    'Record the prescription decision for this consultation before you continue.',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                        fontSize: 14, height: 1.4, color: DoctorTokens.slate)),
              ])),
              const SizedBox(height: 16),
              FilledButton.icon(
                  onPressed: busy ? null : _dispatchRx,
                  icon: const Icon(Icons.receipt_long),
                  label: const Text('Dispatch prescription')),
              const SizedBox(height: 10),
              OutlinedButton.icon(
                  onPressed: busy ? null : _noRx,
                  icon: const Icon(Icons.do_not_disturb_alt_outlined),
                  label: const Text('No prescription needed')),
              const SizedBox(height: 18),
              if (busy)
                const Center(child: CircularProgressIndicator())
              else
                TextButton.icon(
                    onPressed: _rejoin,
                    icon: const Icon(Icons.videocam_outlined),
                    label: const Text('Call dropped? Rejoin call')),
            ])));
  }
}
