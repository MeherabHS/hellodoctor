const fs = require('fs');
let code = fs.readFileSync('scratch/test_multi_rx_upload_suite.js', 'utf8');

code = code.replace(
  'const sandbox = {\n  console: console,',
  `const sandbox = {
  console: console,
  addEventListener: () => {},
  removeEventListener: () => {},`
);

code = code.replace(
  'window: {},\n  showAppToast:',
  `window: {
    addEventListener: () => {},
    removeEventListener: () => {}
  },
  showAppToast:`
);

fs.writeFileSync('scratch/test_multi_rx_upload_suite.js', code, 'utf8');
console.log("✓ Added addEventListener to test sandbox");
