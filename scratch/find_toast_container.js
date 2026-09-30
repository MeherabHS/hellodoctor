const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');
const lines = h.split('\n');
lines.forEach((l, i) => {
  if (l.includes('appToastContainer') || l.includes('app-toast') || l.includes('paidChatBookingModal')) {
    console.log((i + 1) + ': ' + l.trim());
  }
});
