const fs = require('fs');
let code = fs.readFileSync('scratch/test_multi_rx_upload_suite.js', 'utf8');

code = code.replace(
  'createElement: () => ({\n      style: {},',
  `createElement: () => ({
      style: {},
      classList: {
        add: () => {},
        remove: () => {},
        contains: () => false
      },`
);

fs.writeFileSync('scratch/test_multi_rx_upload_suite.js', code, 'utf8');
console.log("✓ Added classList to createElement in test sandbox");
