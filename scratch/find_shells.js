const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');

const shellMatches = h.match(/id="[^"]*Shell[^"]*"/gi) || [];
console.log('Shells found:', shellMatches);

const roleMatches = h.match(/data-role="[^"]*"/gi) || [];
console.log('Roles found:', Array.from(new Set(roleMatches)));

const lines = h.split('\n');
for (let i = 0; i < lines.length; i++) {
  if (lines[i].includes('function switchRole')) {
    console.log('switchRole at line', i + 1);
    console.log(lines.slice(i, i + 35).join('\n'));
    break;
  }
}
