const fs = require('fs');
let code = fs.readFileSync('scratch/test_multi_rx_upload_suite.js', 'utf8');

code = code.replace(
  'removeEventListener: () => {},\n    getElementById:',
  `removeEventListener: () => {},
    createElement: () => ({
      style: {},
      className: '',
      innerHTML: '',
      textContent: '',
      querySelector: () => ({ style: {} }),
      querySelectorAll: () => [],
      appendChild: () => {},
      remove: () => {}
    }),
    body: {
      appendChild: () => {},
      removeChild: () => {}
    },
    getElementById:`
);

fs.writeFileSync('scratch/test_multi_rx_upload_suite.js', code, 'utf8');
console.log("✓ Added createElement and body to sandbox.document");
