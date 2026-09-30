const fs = require('fs');
let content = fs.readFileSync('scratch/test_grievance_suite.js', 'utf8');

// Add assertion that telemetry is NOT visible to patients on the patient end
if (!content.includes('Patient End Privacy Check')) {
  const target = "assert.ok(!html.includes('id=\"pgmRemedySelect\"'), 'Clinical Governance Rule: Patients must NOT select or suggest punitive remedies (pgmRemedySelect must not exist)');";
  const addedCheck = `
// Patient End Privacy & Clinical Experience Check: Raw telemetry must NOT be visible to patients
assert.ok(html.includes('id="pgmTelemetryInspector" aria-hidden="true"') && html.includes('style="display:none;" id="pgmTelemetryInspector"'), 'Patient Privacy Rule: Raw telemetry must NOT be visible on the user/patient end (pgmTelemetryInspector must be hidden)');
assert.ok(html.includes('id="v11TelemetryCard" aria-hidden="true"') && html.includes('style="display:none;" id="v11TelemetryCard"'), 'Patient Privacy Rule: Raw telemetry card must NOT be visible on post-consultation screen (v11TelemetryCard must be hidden)');`;

  content = content.replace(target, target + '\n' + addedCheck);
  fs.writeFileSync('scratch/test_grievance_suite.js', content, 'utf8');
  console.log("✓ Updated test_grievance_suite.js with patient privacy assertion");
}
