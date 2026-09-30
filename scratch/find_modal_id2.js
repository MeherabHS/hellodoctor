const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const targetIdx = 374245;
const chunk = html.substring(targetIdx - 2500, targetIdx);
const modalMatches = chunk.match(/<div[^>]*id="([^"]+)"[^>]*>/g);
console.log("IDs found before pdfUploadedPhotoImg:", modalMatches);
