const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const closeIdx = html.indexOf('closeLabReportModal()');
console.log(html.substring(closeIdx - 600, closeIdx + 100));
