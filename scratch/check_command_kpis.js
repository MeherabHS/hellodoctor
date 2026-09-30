const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const cmdStart = html.indexOf('id="adminPage_command"');
const cmdEnd = html.indexOf('id="adminPage_slots"');
const chunk = html.substring(cmdStart, cmdEnd);

const subs = chunk.match(/<span class="admin-kpi-sub">([^<]+)<\/span>/g);
console.log("Command Center KPIs:", subs);
const vals = chunk.match(/<div class="admin-kpi-val"[^>]*>([^<]+)<\/div>/g);
console.log("Values:", vals);
