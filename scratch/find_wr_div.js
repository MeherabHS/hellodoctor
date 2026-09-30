const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="waitingRoomReportName"');
const divStart = html.lastIndexOf('<div', idx);
const btnEnd = html.indexOf('</button>', idx);
const divEnd = html.indexOf('</div>', btnEnd);

console.log("Found chunk:");
console.log(html.substring(divStart, divEnd + 6));
