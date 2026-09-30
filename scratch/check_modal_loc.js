const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const grvModal = html.indexOf('id="adminGrievanceDetailModal"');
const endGrvModal = html.indexOf('<!-- /#adminGrievanceDetailModal -->', grvModal);
if (endGrvModal !== -1) {
  console.log("adminGrievanceDetailModal ends at:", endGrvModal);
  console.log(html.substring(endGrvModal, endGrvModal + 300));
} else {
  console.log("Checking snippet around adminGrievanceDetailModal...");
  console.log(html.substring(grvModal, grvModal + 400));
}
