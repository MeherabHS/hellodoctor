const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Look for bkash, nagad, payment gateway, confirmPayment, etc.
const paymentMatches = [];
const lines = html.split('\n');
lines.forEach((line, idx) => {
  if (line.includes('confirmPayment') || line.includes('processPayment') || line.includes('payWith') || line.includes('bkash') || line.includes('bKash') || line.includes('nagad') || line.includes('Nagad') || line.includes('mfs') || line.includes('MFS')) {
    if (line.includes('function') || line.includes('onclick') || line.includes('const ') || line.includes('let ')) {
      paymentMatches.push((idx+1) + ': ' + line.trim().substring(0, 120));
    }
  }
});
console.log("Payment related code points:");
console.log(paymentMatches.slice(0, 30).join('\n'));
