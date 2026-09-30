const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const regex = /id="(adminPage_[^"]+)"/g;
let match;
console.log("Admin Pages found:");
while ((match = regex.exec(html)) !== null) {
  console.log(" - " + match[1]);
}

const navRegex = /id="(adminNav-[^"]+)"[^>]*>([\s\S]*?)<\/div>/g;
console.log("\nAdmin Nav Items found:");
while ((match = navRegex.exec(html)) !== null) {
  const text = match[2].replace(/<[^>]+>/g, '').trim().replace(/\s+/g, ' ');
  console.log(` - ${match[1]}: "${text}"`);
}
