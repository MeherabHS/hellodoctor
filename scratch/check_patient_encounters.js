const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const ptStoreIdx = html.indexOf('const adminPatientStore');
if (ptStoreIdx !== -1) {
  console.log("=== adminPatientStore snippet ===");
  console.log(html.substring(ptStoreIdx, ptStoreIdx + 1500));
}
