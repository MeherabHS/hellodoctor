import { tr } from './i18n/core.ts';

type ErasureResult = {
  outcome: 'HARD_DELETED' | 'ANONYMIZED' | 'PENDING';
  job_id?: string;
};

export function dataRightsNotice(result: ErasureResult, subject: 'Patient' | 'Doctor', refreshed = true) {
  let title: string;
  let message: string;
  let level: 'success' | 'warning' = 'success';
  let icon = '🗑️';
  const patient = subject === 'Patient';
  switch (result.outcome) {
    case 'PENDING':
      title = tr('lib.erasurePending');
      message = tr('lib.erasurePendingMessage', { reference: result.job_id || tr('lib.savedRequest') });
      level = 'warning';
      icon = '⏳';
      break;
    case 'HARD_DELETED':
      title = tr(patient ? 'lib.patientDataErased' : 'lib.doctorDataErased');
      message = tr('lib.noRetainedHistory');
      break;
    case 'ANONYMIZED':
      title = tr(patient ? 'lib.patientDataAnonymized' : 'lib.doctorDataAnonymized');
      message = tr(patient ? 'lib.retainedClinicalHistory' : 'lib.retainedPayoutHistory');
      break;
    default:
      throw new Error(tr('lib.unrecognizedErasureOutcome'));
  }
  if (!refreshed) {
    message += tr('lib.dashboardRefreshFailed');
    level = 'warning';
  }
  return { title, message, level, icon };
}
