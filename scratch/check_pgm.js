const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const pgmIdx = html.indexOf('id="patientGrievanceModal"');
console.log('=== patientGrievanceModal snippet ===');
console.log(html.substring(pgmIdx, pgmIdx + 3000));

const v11Idx = html.indexOf('id="view-11"');
console.log('\n=== view-11 snippet ===');
console.log(html.substring(v11Idx, v11Idx + 2000));
