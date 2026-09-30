const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const endGrievances = html.indexOf('</div><!-- /#adminPage_grievances -->');
console.log(html.substring(endGrievances - 100, endGrievances + 200));
