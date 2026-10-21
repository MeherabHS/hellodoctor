import '../../../core/network/api_client.dart';
import '../../../core/network/client_error_reporter.dart';

/// Backs `POST /grievances` (fully real — plan confirms this directly) and
/// resolving the `consultation_id` it requires via the new
/// `GET /appointments/:id/consultation` (closed alongside this milestone,
/// same gap-shape as gap #3's conversation lookup).
class GrievanceRepository {
  GrievanceRepository(this.api);
  final ApiClient api;

  dynamic _data(dynamic r) {
    final body = r.data;
    if (body is Map && body.containsKey('data')) return body['data'];
    return body;
  }

  /// Throws a 404 `ApiException` if no consultation session exists yet for
  /// this appointment (nobody has minted an RTC token for it) — callers
  /// should show this as "nothing to report yet," not a generic error.
  Future<String> resolveConsultationId(String appointmentId) async {
    try {
      final raw =
          _data(await api.get('/appointments/$appointmentId/consultation'));
      return Map<String, dynamic>.from(raw as Map)['id'] as String;
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Grievance', operation: 'resolve_consultation');
      rethrow;
    }
  }

  /// `target`: `'DOCTOR'` or `'SYSTEM'` (`GrievanceTarget`). `idempotent:
  /// true` — a retried double-submit of the same report must not file twice.
  Future<Map<String, dynamic>> submit({
    required String consultationId,
    required String target,
    required String category,
    required String claimSummary,
  }) async {
    try {
      final raw = _data(await api.post('/grievances', idempotent: true, data: {
        'consultation_id': consultationId,
        'target': target,
        'category': category,
        'claim_summary': claimSummary,
      }));
      return Map<String, dynamic>.from(raw as Map);
    } catch (error, stack) {
      ClientErrorReporter.observe(error, stack,
          screen: 'Grievance', operation: 'submit');
      rethrow;
    }
  }
}
