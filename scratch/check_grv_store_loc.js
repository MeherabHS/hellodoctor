const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const grvStoreIdx = html.indexOf('const grievanceRecords');
console.log("grievanceRecords line approx:", html.substring(0, grvStoreIdx).split('\n').length);
console.log(html.substring(grvStoreIdx, grvStoreIdx + 400));
