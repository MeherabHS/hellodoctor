const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('pdfUploadedPhotoImg');
console.log("pdfUploadedPhotoImg found at:", idx);
console.log("Searching backwards for modal start...");
const modalStart = html.lastIndexOf('<div class="inapp-modal-backdrop"', idx) !== -1 
  ? html.lastIndexOf('<div class="inapp-modal-backdrop"', idx)
  : html.lastIndexOf('id="', idx);
console.log(html.substring(modalStart - 100, modalStart + 400));
