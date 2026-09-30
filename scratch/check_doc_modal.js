const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="doctorReportViewerModal"');
console.log("doctorReportViewerModal line approx:", html.substring(0, idx).split('\n').length);
console.log(html.substring(idx, idx + 1800));
