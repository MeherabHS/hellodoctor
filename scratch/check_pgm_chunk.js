const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const pgmTelemIdx = html.indexOf('id="pgmTelemetryInspector"');
console.log(html.substring(pgmTelemIdx - 300, pgmTelemIdx + 2000));
