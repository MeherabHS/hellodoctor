const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="waitingRoomReportName"');
const openDiv = html.lastIndexOf('<div', html.lastIndexOf('<div', idx) - 1);
console.log("Characters:", JSON.stringify(html.substring(openDiv, idx)));
