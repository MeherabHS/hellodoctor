const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const regex = /\.(admin-[a-zA-Z0-9_-]+|badge-[a-zA-Z0-9_-]+)\s*\{/g;
const classes = new Set();
let m;
while ((m = regex.exec(html)) !== null) {
  classes.add(m[1]);
}
console.log("Admin CSS classes found:", Array.from(classes).slice(0, 40));
