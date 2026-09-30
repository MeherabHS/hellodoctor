const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const griStart = html.indexOf('id="adminPage_grievances"');
const griEnd = html.indexOf('id="adminDoctorHistoryModal"');
console.log("adminPage_grievances line approx:", html.substring(0, griStart).split('\n').length);
console.log("adminDoctorHistoryModal line approx:", html.substring(0, griEnd).split('\n').length);
console.log("End of admin pages snippet:");
console.log(html.substring(griEnd - 500, griEnd + 200));
