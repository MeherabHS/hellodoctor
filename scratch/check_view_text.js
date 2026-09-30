const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

[6, 7, 8, 9, 10, 11, 12, 13, 14].forEach(n => {
  const idx = html.indexOf(`id="view-${n}"`);
  if (idx !== -1) {
    const chunk = html.substring(idx, idx + 600);
    const cleaned = chunk.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim();
    console.log(`view-${n}:`, cleaned.substring(0, 120));
  }
});
