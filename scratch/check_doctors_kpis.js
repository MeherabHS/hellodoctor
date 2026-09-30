const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const docStart = html.indexOf('id="adminPage_doctors"');
const docEnd = html.indexOf('id="adminPage_command"');
const chunk = html.substring(docStart, docEnd);

const subs = chunk.match(/<span class="admin-kpi-sub">([^<]+)<\/span>/g);
console.log("Doctor Page KPIs:", subs);
const vals = chunk.match(/<div class="admin-kpi-val"[^>]*>([\s\S]*?)<\/div>/g);
console.log("Values:", vals.map(v => v.replace(/<[^>]+>/g, '').trim()));
