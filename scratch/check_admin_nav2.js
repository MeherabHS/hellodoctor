const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const navStart = html.indexOf('id="adminNav-doctors"');
if (navStart !== -1) {
  console.log("=== adminNav-doctors context ===");
  console.log(html.substring(navStart - 300, navStart + 1500));
}
