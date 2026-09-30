const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('waitingRoomReportName');
console.log(html.substring(idx - 250, idx + 350));
