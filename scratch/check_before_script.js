const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const grvModal = html.indexOf('id="adminGrievanceDetailModal"');
const scriptTag = html.indexOf('<script', grvModal);
console.log("HTML before script tag snippet:");
console.log(html.substring(scriptTag - 400, scriptTag));
