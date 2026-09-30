const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const cmdStart = html.indexOf('id="adminPage_command"');
const cmdEnd = html.indexOf('id="adminPage_slots"');
console.log("=== adminPage_command ===");
console.log(html.substring(cmdStart, cmdStart + 2000));
