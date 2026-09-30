const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const lines = html.split('\n');
console.log("Total lines:", lines.length);
console.log(lines.slice(lines.length - 120).join('\n'));
