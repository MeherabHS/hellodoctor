const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const regex = /id="(view-\d+)"/g;
let m;
while ((m = regex.exec(html)) !== null) {
  const start = m.index;
  const chunk = html.substring(start, start + 300);
  const titleMatch = chunk.match(/<h1>([^<]+)<\/h1>/) || chunk.match(/class="[^"]*title[^"]*">([^<]+)</) || chunk.match(/<h2>([^<]+)<\/h2>/);
  console.log(m[1], titleMatch ? titleMatch[1].trim() : '');
}
