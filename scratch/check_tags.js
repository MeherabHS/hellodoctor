const fs = require('fs');
const html = fs.readFileSync('scratch/generated_admin_shell.html', 'utf8');
const tags = ['div', 'header', 'aside', 'main', 'nav', 'table', 'tbody', 'thead', 'tr', 'th', 'td', 'button', 'span', 'svg'];
let mismatches = 0;
tags.forEach(t => {
  const o = (html.match(new RegExp(`<${t}[\\s>]`, 'gi')) || []).length;
  const c = (html.match(new RegExp(`</${t}>`, 'gi')) || []).length;
  if (o !== c) {
    console.log(`Tag mismatch: <${t}> open=${o}, close=${c}`);
    mismatches++;
  } else {
    console.log(`✓ <${t}>: ${o} opens, ${c} closes`);
  }
});
if (mismatches === 0) {
  console.log('ALL TAGS PERFECTLY BALANCED!');
} else {
  console.log(`Found ${mismatches} mismatches.`);
}
