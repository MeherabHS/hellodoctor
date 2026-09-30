const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnSelect = html.indexOf('function handlePatientFileSelection');
const fnRemove = html.indexOf('function removeAttachedReport');
console.log(html.substring(fnSelect, fnRemove + 400));
