const fs = require('fs');

// 1. Ensure prototype/index.html exists as a clone of index.html
if (!fs.existsSync('prototype')) {
  fs.mkdirSync('prototype');
}
fs.copyFileSync('index.html', 'prototype/index.html');
console.log("✓ Copied index.html to prototype/index.html");

// 2. Update all scratch test files to check both paths gracefully
const files = fs.readdirSync('scratch');
files.forEach(f => {
  if (f.endsWith('.js') && f.startsWith('test_')) {
    const p = 'scratch/' + f;
    let code = fs.readFileSync(p, 'utf8');
    if (code.includes("'prototype/index.html'")) {
      code = code.replace(
        /const\s+html\s*=\s*fs\.readFileSync\(['"]prototype\/index\.html['"],\s*['"]utf8['"]\);/g,
        "const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';\nconst html = fs.readFileSync(htmlFile, 'utf8');"
      );
      code = code.replace(
        /fs\.readFileSync\(['"]prototype\/index\.html['"],\s*['"]utf8['"]\)/g,
        "(fs.existsSync('index.html') ? fs.readFileSync('index.html', 'utf8') : fs.readFileSync('prototype/index.html', 'utf8'))"
      );
      fs.writeFileSync(p, code, 'utf8');
      console.log(`✓ Updated path handling in ${f}`);
    }
  }
});
