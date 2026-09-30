const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const navStart = html.indexOf('<div class="admin-sidebar-nav">');
if (navStart !== -1) {
  console.log("=== admin-sidebar-nav snippet ===");
  console.log(html.substring(navStart, navStart + 1800));
}
