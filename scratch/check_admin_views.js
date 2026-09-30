const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const views = [...html.matchAll(/id="(adminPage_[^"]+)"([^>]*)/g)];
console.log('Found ' + views.length + ' admin pages:');
let failed = 0;
views.forEach(v => {
  const hasNone = v[2].includes('display:none') || v[2].includes('display: none');
  console.log('  ' + v[1] + ': ' + (hasNone ? '❌ FAIL: display:none' : '✓ PASS: clean'));
  if (hasNone) failed++;
});

if (failed > 0) {
  console.error(`Found ${failed} views with conflicting inline styles!`);
  process.exit(1);
} else {
  console.log('All admin page views are 100% clean!');
}
