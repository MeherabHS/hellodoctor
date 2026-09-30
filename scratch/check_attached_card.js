const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="patientAttachedDocCard"');
console.log(html.substring(idx - 100, idx + 800));
