const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

[6, 7, 8, 9, 10, 11, 12, 13, 14].forEach(n => {
  const idx = html.indexOf(`id="view-${n}"`);
  if (idx !== -1) {
    const chunk = html.substring(idx, idx + 400);
    const m = chunk.match(/<h[1-4][^>]*>([^<]+)<\/h[1-4]>/);
    console.log(`view-${n}:`, m ? m[1].trim() : 'no header tag found');
  }
});
