const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const adminShellIdx = html.indexOf('id="adminPortalShell"');
const patientPart = html.substring(0, adminShellIdx);

const matches = [];
const lines = patientPart.split('\n');
lines.forEach((l, i) => {
  if (l.toLowerCase().includes('telemetry') || l.toLowerCase().includes('webrtc connection') || l.toLowerCase().includes('tamper-proof')) {
    matches.push((i+1) + ': ' + l.trim());
  }
});
console.log("Matches in patient app portion:");
console.log(matches.join('\n'));
