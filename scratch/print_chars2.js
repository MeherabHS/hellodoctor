const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('<div style="flex:1; min-width:0;">\n                <div style="font-size:11.5px; font-weight:750; color:#065F46; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;" id="waitingRoomReportName"');
console.log(JSON.stringify(html.substring(idx - 250, idx)));
