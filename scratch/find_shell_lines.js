const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');
const lines = h.split('\n');
lines.forEach((l, i) => {
  if (l.includes('id="doctorAppShell"') || l.includes('id="doctorWebShell"') || l.includes('id="adminPortalShell"')) {
    console.log((i + 1) + ': ' + l);
  }
});
