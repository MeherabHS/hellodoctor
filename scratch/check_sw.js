const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const swIdx = html.indexOf('function switchAdminPage');
console.log(html.substring(swIdx, swIdx + 1200));
