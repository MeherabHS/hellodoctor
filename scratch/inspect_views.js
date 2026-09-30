const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');

const views = [];
const regex = /<div class="screen([^"]*)" id="view-(\d+)"/g;
let match;
while ((match = regex.exec(h)) !== null) {
  views.push({ id: match[2], class: match[1] });
}
console.log('Total views in phone shell:', views.length);
console.log(views);

const doctorViews = [];
const docRegex = /<div class="screen([^"]*)" id="docView-(\d+)"/g;
while ((match = docRegex.exec(h)) !== null) {
  doctorViews.push({ id: match[2], class: match[1] });
}
console.log('Total doc views:', doctorViews.length);
console.log(doctorViews);
