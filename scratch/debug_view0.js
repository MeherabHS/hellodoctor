const fs = require('fs');
const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');

const view0Start = html.indexOf('id="view-0"');
const view0End = html.indexOf('id="view-1"', view0Start);
const view0Content = html.substring(view0Start, view0End);

console.log('view0 length:', view0Content.length);
console.log('coreServicesIdx:', view0Content.indexOf('core-services-2x2'));
console.log('helplineIdx:', view0Content.indexOf('id="patientEmergencyHelplineStrip"'));
console.log('upcomingIdx:', view0Content.indexOf('Upcoming Consultation'));
console.log('view0 snippet around upcoming:');
const hIdx = view0Content.indexOf('patientEmergencyHelplineStrip');
if (hIdx !== -1) {
  console.log(view0Content.substring(hIdx - 100, hIdx + 400));
}
