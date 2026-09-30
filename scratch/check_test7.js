const fs = require('fs');
const content = fs.readFileSync('scratch/test_grievance_suite.js', 'utf8');

const t7Idx = content.indexOf('Test 7:');
console.log(content.substring(t7Idx, t7Idx + 1500));
