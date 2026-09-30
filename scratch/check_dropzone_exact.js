const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="patientUploadDropzone"');
console.log(html.substring(idx - 200, idx + 1200));
