const fs = require('fs');
const html = fs.readFileSync('index.html', 'utf8');

console.log('=== PATIENT VIEWS ===');
for (let i = 0; i <= 15; i++) {
  const v = 'view-' + i;
  const start = html.indexOf('id="' + v + '"');
  if (start !== -1) {
    const end = html.indexOf('id="view-' + (i + 1) + '"', start);
    const chunk = html.substring(start, end !== -1 ? end : start + 3000);
    const m = chunk.match(/<h[1-4][^>]*>(.*?)<\/h[1-4]>/i) || chunk.match(/class="[^"]*title[^"]*">([^<]+)</i);
    const title = m ? m[1].replace(/<[^>]+>/g, '').trim() : 'Unknown';
    // Look for comments or buttons
    const desc = chunk.match(/<!--\s*(.*?)\s*-->/);
    console.log(`${v}: "${title}" ${desc ? '[' + desc[1] + ']' : ''}`);
  }
}

console.log('\n=== DOCTOR VIEWS ===');
for (let i = 0; i <= 8; i++) {
  const v = 'doc-view-' + i;
  const start = html.indexOf('id="' + v + '"');
  if (start !== -1) {
    const end = html.indexOf('id="doc-view-' + (i + 1) + '"', start);
    const chunk = html.substring(start, end !== -1 ? end : start + 3000);
    const m = chunk.match(/<h[1-4][^>]*>(.*?)<\/h[1-4]>/i) || chunk.match(/class="[^"]*title[^"]*">([^<]+)</i);
    const title = m ? m[1].replace(/<[^>]+>/g, '').trim() : 'Unknown';
    const desc = chunk.match(/<!--\s*(.*?)\s*-->/);
    console.log(`${v}: "${title}" ${desc ? '[' + desc[1] + ']' : ''}`);
  }
}

console.log('\n=== ADMIN PAGES ===');
const adminPages = ['command', 'doctors', 'patients', 'bmdc', 'slots', 'compliance', 'logs', 'grievances', 'finance'];
adminPages.forEach(p => {
  const id = 'adminPage_' + p;
  const start = html.indexOf('id="' + id + '"');
  if (start !== -1) {
    const chunk = html.substring(start, start + 800);
    const m = chunk.match(/<h[1-4][^>]*>(.*?)<\/h[1-4]>/i);
    console.log(`${id}: "${m ? m[1].replace(/<[^>]+>/g, '').trim() : 'Active'}"`);
  }
});
