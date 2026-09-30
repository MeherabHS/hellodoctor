const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const tBodyStart = html.indexOf('id="adminDoctorsTableBody"');
console.log(html.substring(tBodyStart, tBodyStart + 1500));
