const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('patientAttachedName');
console.log("patientAttachedName context:");
console.log(html.substring(idx - 350, idx + 400));
