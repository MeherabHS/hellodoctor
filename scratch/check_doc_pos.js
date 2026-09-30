const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const docPageIdx = html.indexOf('id="adminPage_doctors"');
console.log("adminPage_doctors line approx:", html.substring(0, docPageIdx).split('\n').length);
console.log(html.substring(docPageIdx - 200, docPageIdx + 500));
