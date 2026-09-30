const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const modalStart = html.indexOf('id="adminPatientDossierModal"');
if (modalStart !== -1) {
  console.log("=== adminPatientDossierModal snippet ===");
  console.log(html.substring(modalStart, modalStart + 1500));
}

const fnDossier = html.indexOf('function openAdminPatientDossierModal');
if (fnDossier !== -1) {
  console.log("=== openAdminPatientDossierModal snippet ===");
  console.log(html.substring(fnDossier, fnDossier + 1500));
}
