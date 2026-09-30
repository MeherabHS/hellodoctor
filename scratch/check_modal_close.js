const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const grvModal = html.indexOf('id="adminGrievanceDetailModal"');
const afterModal = html.indexOf('id="globalLoadingOverlay"', grvModal);
console.log(html.substring(afterModal - 300, afterModal + 100));
