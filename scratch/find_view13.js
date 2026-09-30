const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');
const lines = h.split('\n');
lines.forEach((l, i) => {
  if (l.includes('id="view-13"') || l.includes('<!-- SCREEN 13') || (l.includes('id="view-') && i > 5500)) {
    console.log((i + 1) + ': ' + l);
  }
});
