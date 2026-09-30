const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const pStart = html.indexOf('id="adminPage_patients"');
const pEnd = html.indexOf('id="adminPage_bmdc"');
console.log("=== adminPage_patients section (first 1000 chars) ===");
console.log(html.substring(pStart, pStart + 1000));

// Check openAdminPatientHistoryModal or similar
const fnPt = html.indexOf('openAdminPatient');
if (fnPt !== -1) {
  console.log("=== openAdminPatient snippet ===");
  console.log(html.substring(fnPt, fnPt + 1000));
}
