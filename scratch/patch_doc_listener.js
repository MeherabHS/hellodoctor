const fs = require('fs');
let code = fs.readFileSync('scratch/test_multi_rx_upload_suite.js', 'utf8');

code = code.replace(
  'document: {\n    getElementById:',
  `document: {
    addEventListener: () => {},
    removeEventListener: () => {},
    getElementById:`
);

fs.writeFileSync('scratch/test_multi_rx_upload_suite.js', code, 'utf8');
console.log("✓ Added addEventListener to sandbox.document");
