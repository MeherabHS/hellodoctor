const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const regex = /adminDoctorHistoryStore\[[^\]]+\]/g;
let m;
while ((m = regex.exec(html)) !== null) {
  console.log(m[0]);
}
