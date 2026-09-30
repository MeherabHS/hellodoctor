const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const fnViewer = html.indexOf('function openDoctorReportViewer');
if (fnViewer !== -1) {
  console.log("=== openDoctorReportViewer snippet ===");
  console.log(html.substring(fnViewer, fnViewer + 1500));
}

const modalViewer = html.indexOf('id="doctorReportViewerModal"');
if (modalViewer !== -1) {
  console.log("\n=== doctorReportViewerModal snippet ===");
  console.log(html.substring(modalViewer, modalViewer + 1000));
}
