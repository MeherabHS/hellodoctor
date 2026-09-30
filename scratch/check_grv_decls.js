const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const matches = [];
const lines = html.split('\n');
lines.forEach((l, i) => {
  if (l.includes('grievance') || l.includes('Grievance')) {
    if (l.includes('const ') || l.includes('let ') || l.includes('var ') || l.includes('function ')) {
      matches.push((i+1) + ': ' + l.trim());
    }
  }
});
console.log(matches.slice(0, 30).join('\n'));
