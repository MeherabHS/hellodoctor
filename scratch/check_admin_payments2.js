const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Let's inspect adminPage_doctors
const docStart = html.indexOf('id="adminPage_doctors"');
const docEnd = html.indexOf('id="adminPage_command"');
console.log("=== adminPage_doctors section (first 1500 chars) ===");
console.log(html.substring(docStart, docStart + 1500));

// Let's inspect where payments, payouts, transactions are declared in javascript
console.log("\n=== JavaScript Payment Data / Functions ===");
const jsRegex = /(function\s+[a-zA-Z0-9_]*(?:Payout|Payment|Transaction|Escrow|Ledger)[a-zA-Z0-9_]*|const\s+[a-zA-Z0-9_]*(?:Payout|Payment|Transaction|Escrow|Ledger)[a-zA-Z0-9_]*|let\s+[a-zA-Z0-9_]*(?:Payout|Payment|Transaction|Escrow|Ledger)[a-zA-Z0-9_]*|var\s+[a-zA-Z0-9_]*(?:Payout|Payment|Transaction|Escrow|Ledger)[a-zA-Z0-9_]*)/g;
let m;
while ((m = jsRegex.exec(html)) !== null) {
  console.log("Found:", m[1]);
}
