const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnSubmit = html.indexOf('function submitPreConsultReportAndJoin');
if (fnSubmit !== -1) {
  console.log("=== submitPreConsultReportAndJoin snippet ===");
  console.log(html.substring(fnSubmit, fnSubmit + 1200));
}
