const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Let's inspect adminPage_doctors table and search
const docStart = html.indexOf('id="adminPage_doctors"');
const docEnd = html.indexOf('id="adminPage_command"');
const doctorsHtml = html.substring(docStart, docEnd);

// Find tables or cards in adminPage_doctors
console.log("=== adminPage_doctors structure ===");
const lines = doctorsHtml.split('\n');
lines.forEach(l => {
  if (l.includes('<table') || l.includes('<thead') || l.includes('<tr') || l.includes('input') || l.includes('filter') || l.includes('Search') || l.includes('search')) {
    console.log(l.trim().substring(0, 100));
  }
});

// Let's check executeAdminBatchPayout implementation
const fnBatch = html.indexOf('function executeAdminBatchPayout');
if (fnBatch !== -1) {
  console.log("\n=== executeAdminBatchPayout snippet ===");
  console.log(html.substring(fnBatch, fnBatch + 1200));
}

// Let's check adminDoctorHistoryModal implementation
const docHist = html.indexOf('id="adminDoctorHistoryModal"');
if (docHist !== -1) {
  console.log("\n=== adminDoctorHistoryModal snippet ===");
  console.log(html.substring(docHist, docHist + 1500));
}
