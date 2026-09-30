const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const telemStart = html.indexOf('id="pgmTelemetryInspector"');
if (telemStart !== -1) {
  console.log("=== pgmTelemetryInspector snippet ===");
  console.log(html.substring(telemStart - 100, telemStart + 1500));
}

const v11Telem = html.indexOf('id="v11TelemetryCard"');
if (v11Telem !== -1) {
  console.log("=== v11TelemetryCard snippet ===");
  console.log(html.substring(v11Telem - 100, v11Telem + 800));
}
