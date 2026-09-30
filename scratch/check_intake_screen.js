const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('patientUploadDropzone');
console.log("Line approx of patientUploadDropzone:", html.substring(0, idx).split('\n').length);
console.log(html.substring(idx - 600, idx + 400));
