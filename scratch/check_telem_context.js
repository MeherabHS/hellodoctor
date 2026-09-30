const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const pgmTelemIdx = html.indexOf('id="pgmTelemetryInspector"');
console.log("=== pgmTelemetryInspector context ===");
console.log(html.substring(pgmTelemIdx - 150, pgmTelemIdx + 400));

const v11TelemIdx = html.indexOf('id="v11TelemetryCard"');
console.log("\n=== v11TelemetryCard context ===");
console.log(html.substring(v11TelemIdx - 150, v11TelemIdx + 400));
