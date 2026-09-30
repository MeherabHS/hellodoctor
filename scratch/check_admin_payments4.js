const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Let's check openAdminDoctorHistoryModal
const fnHist = html.indexOf('function openAdminDoctorHistoryModal');
if (fnHist !== -1) {
  console.log("=== openAdminDoctorHistoryModal snippet ===");
  console.log(html.substring(fnHist, fnHist + 1500));
}

// Let's check grievance records or payment records in JS
const griMatch = html.indexOf('const grievanceRecords');
if (griMatch !== -1) {
  console.log("=== grievanceRecords snippet ===");
  console.log(html.substring(griMatch, griMatch + 1000));
}

// Let's check consultation records or payment records in JS
const conMatch = html.indexOf('const consultationTelemetryStore');
if (conMatch !== -1) {
  console.log("=== consultationTelemetryStore snippet ===");
  console.log(html.substring(conMatch, conMatch + 1200));
}
