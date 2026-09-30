const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');
const lines = h.split('\n');
lines.forEach((l, i) => {
  if (l.includes('Booking') || l.includes('book') || l.includes('switchScreen(4)') || l.includes('switchScreen(10)')) {
    if (l.includes('function ') || l.includes('onclick=')) {
      console.log((i + 1) + ': ' + l.trim());
    }
  }
});
