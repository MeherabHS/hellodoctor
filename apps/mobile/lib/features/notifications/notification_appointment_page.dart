import 'package:flutter/material.dart';
import '../../core/network/api_client.dart';
import '../../core/l10n/doctor_locale.dart';

/// A focused, authenticated appointment destination. Reads current server state
/// on opening/retry; never auto-enters a clinical room or initiates a payment.
class NotificationAppointmentPage extends StatefulWidget {
  const NotificationAppointmentPage(
      {super.key, required this.api, required this.id});
  final ApiClient api;
  final String id;
  @override
  State<NotificationAppointmentPage> createState() => _AppointmentState();
}

class _AppointmentState extends State<NotificationAppointmentPage> {
  late Future<Map<String, dynamic>> _future;
  Future<Map<String, dynamic>> _load() async {
    final response = await widget.api.get('/appointments/${widget.id}');
    final data = Map<String, dynamic>.from(response.data['data'] as Map);
    if (data['id'] != widget.id) {
      throw const FormatException('Appointment mismatch');
    }
    return data;
  }

  @override
  void initState() {
    super.initState();
    _future = _load();
  }

  void _refresh() => setState(() => _future = _load());
  @override
  Widget build(BuildContext context) {
    final l10n = doctorL10n(context);
    final statuses = {
      'PENDING_PAYMENT': l10n.appointmentStatusPendingPayment,
      'CONFIRMED': l10n.appointmentStatusConfirmed,
      'WAITING': l10n.appointmentStatusWaiting,
      'IN_CONSULTATION': l10n.appointmentStatusInConsultation,
      'COMPLETED': l10n.appointmentStatusCompleted,
      'CANCELLED': l10n.appointmentStatusCancelled,
      'EXPIRED': l10n.appointmentStatusExpired,
      'REFUNDED': l10n.appointmentStatusRefunded,
      'DISPUTED': l10n.appointmentStatusDisputed,
      'PREMATURE_TERMINATION': l10n.appointmentStatusPrematureTermination,
    };
    return Scaffold(
        appBar: AppBar(title: Text(l10n.appointmentTitle), actions: [
          IconButton(
              tooltip: l10n.refreshButton,
              onPressed: _refresh,
              icon: const Icon(Icons.refresh))
        ]),
        body: FutureBuilder<Map<String, dynamic>>(
            future: _future,
            builder: (context, snapshot) {
              if (snapshot.connectionState != ConnectionState.done) {
                return const Center(child: CircularProgressIndicator());
              }
              if (snapshot.hasError) {
                return Center(
                    child: Padding(
                        padding: const EdgeInsets.all(24),
                        child:
                            Column(mainAxisSize: MainAxisSize.min, children: [
                          Text(l10n.notificationDestinationError),
                          TextButton(
                              onPressed: _refresh,
                              child: Text(l10n.retryButton))
                        ])));
              }
              final data = snapshot.data!;
              return ListView(padding: const EdgeInsets.all(24), children: [
                Text(l10n.appointmentReference,
                    style: Theme.of(context).textTheme.labelLarge),
                const SizedBox(height: 8),
                Text(data['appointment_number']?.toString() ?? '',
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 24),
                Text(statuses[data['status']] ?? l10n.appointmentTitle,
                    style: Theme.of(context).textTheme.titleMedium),
              ]);
            }));
  }
}
