const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const grvModal = html.indexOf('id="adminGrievanceDetailModal"');
const closeFn = html.indexOf('function closeAdminGrievanceDetail', grvModal);
console.log(html.substring(closeFn - 300, closeFn + 100));
