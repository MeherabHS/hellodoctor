const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

console.log("=== Booking / Intake Upload (around line 7510) ===");
const idx1 = html.indexOf('patientUploadDropzone');
console.log(html.substring(idx1 - 200, idx1 + 1500));

console.log("\n=== Scheduled Consultation Screen (around line 5720) ===");
const idx2 = html.indexOf('Thyroid_Function_Pro');
console.log(html.substring(idx2 - 300, idx2 + 1000));
