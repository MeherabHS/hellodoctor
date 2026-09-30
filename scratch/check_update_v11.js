const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnV11 = html.indexOf('function updateView11TelemetryCard');
console.log(html.substring(fnV11, fnV11 + 600));
