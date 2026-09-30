const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const metaStart = html.indexOf('const adminPageMeta');
if (metaStart !== -1) {
  console.log("=== adminPageMeta ===");
  console.log(html.substring(metaStart, metaStart + 800));
}
