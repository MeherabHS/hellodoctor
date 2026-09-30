const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnUi = html.indexOf('function updatePatientGrievanceTelemetryUI');
console.log(html.substring(fnUi, fnUi + 1200));
