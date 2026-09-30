const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const s1Start = html.indexOf('<script');
const s1End = html.indexOf('</script>', s1Start);
const s2Start = html.indexOf('<script', s1End);
const s2End = html.indexOf('</script>', s2Start);

console.log("Script 1:", s1Start, "to", s1End, "(length:", s1End - s1Start, ")");
console.log("Script 2:", s2Start, "to", s2End, "(length:", s2End - s2Start, ")");

console.log("\nWhat is between Script 1 and Script 2?");
console.log(html.substring(s1End, s2Start + 10));
