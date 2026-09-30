const fs = require('fs');
let code = fs.readFileSync('scratch/test_multi_rx_upload_suite.js', 'utf8');

code = code.replace(
  'innerHTML: \'\',\n      textContent:',
  `innerHTML: '',
      appendChild: () => {},
      removeChild: () => {},
      textContent:`
);

fs.writeFileSync('scratch/test_multi_rx_upload_suite.js', code, 'utf8');
console.log("✓ Added appendChild to getOrCreate mock elements");
