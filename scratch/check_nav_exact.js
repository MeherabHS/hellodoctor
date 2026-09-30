const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const navStart = html.indexOf('<nav class="admin-sidebar-nav">');
const navEnd = html.indexOf('</nav>', navStart);
console.log(html.substring(navStart, navEnd + 6));
