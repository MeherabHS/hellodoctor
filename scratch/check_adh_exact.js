const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const idx = html.indexOf('id="adminDoctorHistoryModal"');
console.log(html.substring(idx, idx + 1000));
