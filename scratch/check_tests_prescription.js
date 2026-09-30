const fs = require('fs');

const files = [
  'scratch/test_admin_suite.js',
  'scratch/test_grievance_suite.js',
  'scratch/test_admin_dom.js',
  'scratch/test_admin_logs.js',
  'scratch/test_admin_searchbars.js',
  'scratch/test_admin_runtime_sim.js',
  'scratch/test_admin_finance_suite.js'
];

files.forEach(f => {
  if (fs.existsSync(f)) {
    const c = fs.readFileSync(f, 'utf8');
    if (c.includes('waitingRoomReportName') || c.includes('patientAttached') || c.includes('Prescription') || c.includes('prescription')) {
      console.log(`Found references in ${f}`);
    }
  }
});
