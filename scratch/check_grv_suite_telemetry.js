const fs = require('fs');
const content = fs.readFileSync('scratch/test_grievance_suite.js', 'utf8');

const lines = content.split('\n');
const domChecks = lines.filter(l => l.includes('pgm') || l.includes('v11') || l.includes('Telemetry') || l.includes('telemetry'));
console.log(domChecks.join('\n'));
