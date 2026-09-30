const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const v8Start = html.indexOf('id="view-8"');
if (v8Start !== -1) {
  console.log("=== view-8 snippet ===");
  console.log(html.substring(v8Start, v8Start + 1500));
}

// And check switchAdminPage
const fnSwitch = html.indexOf('function switchAdminPage');
if (fnSwitch !== -1) {
  console.log("=== switchAdminPage snippet ===");
  console.log(html.substring(fnSwitch, fnSwitch + 800));
}
