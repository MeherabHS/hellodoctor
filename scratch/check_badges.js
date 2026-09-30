const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const regex = /\.(badge-[a-zA-Z0-9_-]+)/g;
const bClasses = new Set();
let m;
while ((m = regex.exec(html)) !== null) {
  bClasses.add(m[1]);
}
console.log("Badge classes:", Array.from(bClasses));
