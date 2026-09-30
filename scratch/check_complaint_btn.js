const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('patientComplaintInput');
console.log(html.substring(idx, idx + 1200));
