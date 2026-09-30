const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnSelect = html.indexOf('function handlePatientFileSelection');
if (fnSelect !== -1) {
  console.log("=== handlePatientFileSelection snippet ===");
  console.log(html.substring(fnSelect, fnSelect + 1500));
}

const fnPreset = html.indexOf('function selectPresetReport');
if (fnPreset !== -1) {
  console.log("\n=== selectPresetReport snippet ===");
  console.log(html.substring(fnPreset, fnPreset + 1000));
}
