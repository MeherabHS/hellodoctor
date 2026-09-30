const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('patientUploadDropzone');
const afterIdx = html.indexOf('confirmPreConsultBooking', idx);
if (afterIdx !== -1) {
  console.log("=== confirmPreConsultBooking snippet ===");
  console.log(html.substring(afterIdx - 200, afterIdx + 600));
} else {
  console.log("Searching for next button after dropzone...");
  console.log(html.substring(idx + 600, idx + 1800));
}
