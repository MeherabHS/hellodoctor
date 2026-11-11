type ErasureResult = {
  outcome: 'HARD_DELETED' | 'ANONYMIZED' | 'PENDING';
  job_id?: string;
};

export function dataRightsNotice(result: ErasureResult, subject: 'Patient' | 'Doctor', refreshed = true) {
  let title: string;
  let message: string;
  let level: 'success' | 'warning' = 'success';
  let icon = '🗑️';
  switch (result.outcome) {
    case 'PENDING':
      title = 'Erasure Pending';
      message = `The request is saved. Deactivation or required cleanup is still pending and will retry automatically. Reference: ${result.job_id || 'saved request'}`;
      level = 'warning';
      icon = '⏳';
      break;
    case 'HARD_DELETED':
      title = `${subject} Data Erased`;
      message = 'No retained history — the profile was permanently deleted.';
      break;
    case 'ANONYMIZED':
      title = `${subject} Data Anonymized`;
      message = subject === 'Patient'
        ? 'Retained clinical/financial history — personal fields were scrubbed, the record stays on file.'
        : 'Retained consultation/payout history — personal fields were scrubbed, the record stays on file.';
      break;
    default:
      throw new Error('Unrecognized erasure outcome. Refresh to check the request status.');
  }
  if (!refreshed) {
    message += ' Dashboard refresh failed; reload to update the displayed records.';
    level = 'warning';
  }
  return { title, message, level, icon };
}
