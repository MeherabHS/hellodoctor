const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="reportScrollArea"');
console.log(html.substring(idx - 1500, idx));
