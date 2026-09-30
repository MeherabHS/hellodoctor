const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const matches = [];
const lines = html.split('\n');
lines.forEach((l, i) => {
  if (l.includes('renderPatientPrescriptionGallery') || l.includes('updateWaitingRoomPrescriptionGallery') || l.includes('openMultiPagePrescriptionViewer')) {
    matches.push((i+1) + ': ' + l.trim());
  }
});
console.log("Prescription gallery function references:");
console.log(matches.join('\n'));
