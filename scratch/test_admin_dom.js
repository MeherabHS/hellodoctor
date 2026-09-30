const fs = require('fs');

// Simple DOM Mock testing
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Verify key element IDs exist in HTML
const requiredElementIds = [
  'btnRoleAdmin',
  'adminPortalShell',
  'adminNav-command',
  'adminNav-doctors',
  'adminNav-patients',
  'adminNav-bmdc',
  'adminNav-slots',
  'adminNav-compliance',
  'adminPage_command',
  'adminPage_doctors',
  'adminPage_patients',
  'adminPage_bmdc',
  'adminPage_slots',
  'adminPage_compliance',
  'adminDoctorsTableBody',
  'adminPatientsTableBody',
  'adminDoctorHistoryModal',
  'adhDocAvatar',
  'adhDocName',
  'adhDocBmdc',
  'adhDocMeta',
  'adhKpiVisits',
  'adhKpiChats',
  'adhKpiGross',
  'adhKpiDisbursed',
  'adhEncountersTableBody',
  'adminPatientDossierModal',
  'apdPatAvatar',
  'apdPatName',
  'apdPatId',
  'apdPatChatBadge',
  'apdPatMeta',
  'apdVitBp',
  'apdVitPulse',
  'apdVitCohort',
  'apdVitLabs',
  'apdHistoryTableBody'
];

console.log('Checking required DOM elements for Admin Portal & Modals:');
let missing = 0;
requiredElementIds.forEach(id => {
  const hasId = html.includes(`id="${id}"`);
  if (!hasId) {
    console.error(`- Missing element with ID: #${id}`);
    missing++;
  } else {
    console.log(`- #${id}: Found ✓`);
  }
});

if (missing === 0) {
  console.log(`\nAll ${requiredElementIds.length} critical DOM elements verified in prototype/index.html!`);
} else {
  console.error(`\nFailed with ${missing} missing elements.`);
  process.exit(1);
}
