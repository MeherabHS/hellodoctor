const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const pgmTelemIdx = html.indexOf('id="pgmTelemetryInspector"');
const submitBtnIdx = html.indexOf('Submit Official Grievance to Admin', pgmTelemIdx);
console.log(html.substring(submitBtnIdx - 500, submitBtnIdx + 200));
