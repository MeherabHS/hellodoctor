const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnSubmit = html.indexOf('function submitPatientGrievance');
console.log(html.substring(fnSubmit, fnSubmit + 1800));
