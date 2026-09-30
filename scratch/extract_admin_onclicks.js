const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');
const adminStart = html.indexOf('id="adminPortalShell"');
const adminEnd = html.indexOf('</main>');
const adminChunk = html.substring(adminStart, adminEnd);
const matches = [...adminChunk.matchAll(/onclick="([^"]+)"/g)].map(m => m[1]);
console.log(JSON.stringify([...new Set(matches)], null, 2));
