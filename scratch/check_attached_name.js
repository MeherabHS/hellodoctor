const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('patientAttachedName');
console.log(html.substring(idx - 300, idx + 1000));
