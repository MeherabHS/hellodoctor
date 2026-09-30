const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="pdfUploadedPhotoContainer"');
if (idx !== -1) {
  console.log("=== pdfUploadedPhotoContainer snippet ===");
  console.log(html.substring(idx - 200, idx + 1000));
}
