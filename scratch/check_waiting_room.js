const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('waitingRoomReportName');
console.log("waitingRoomReportName line approx:", html.substring(0, idx).split('\n').length);
console.log(html.substring(idx - 400, idx + 800));
