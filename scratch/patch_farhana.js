const fs = require('fs');
const file = 'prototype/index.html';
let html = fs.readFileSync(file, 'utf8');

const target = `<strong style="color:#0F172A; font-size:13px;">Dr. Farhana Yesmin</strong>
                          <div style="font-size:10.5px; color:#64748B;">Gynaecology &amp; Obstetrics</div>`;

const replacement = `<strong style="color:#0F172A; font-size:13px;">Dr. Farhana Yesmin</strong>
                          <div style="font-size:10.5px; color:#64748B;">Gynaecology &amp; Obstetrics • Uttara, Dhaka</div>
                          <div style="font-size:10px; color:#2563EB; font-family:monospace; margin-top:1px;">📞 +880 1912-778899</div>`;

if (html.includes(target)) {
  html = html.replace(target, replacement);
  fs.writeFileSync(file, html, 'utf8');
  console.log('Successfully enriched Farhana row!');
} else {
  console.log('Target not found');
}
