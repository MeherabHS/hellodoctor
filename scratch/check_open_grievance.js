const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnOpen = html.indexOf('function openPatientGrievanceModal');
console.log(html.substring(fnOpen, fnOpen + 1200));
