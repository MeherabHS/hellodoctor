const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

// Find where window.patientPrescriptionImages = patientPrescriptionImages; is in Script 2
const danglingBlock = `window.patientPrescriptionImages = patientPrescriptionImages;
      window.handlePatientFileSelection = handlePatientFileSelection;
      window.removePatientPrescriptionImage = removePatientPrescriptionImage;
      window.renderPatientPrescriptionGallery = renderPatientPrescriptionGallery;
      window.updateWaitingRoomPrescriptionGallery = updateWaitingRoomPrescriptionGallery;
      window.openMultiPagePrescriptionViewer = openMultiPagePrescriptionViewer;
      window.switchPrescriptionViewerPage = switchPrescriptionViewerPage;`;

if (html.includes(danglingBlock)) {
  html = html.replace(danglingBlock, '');
  console.log("✓ Removed dangling assignment from Script 2");
}

// In Script 1, right after switchPrescriptionViewerPage definition, attach them to window
const attachBlock = `
window.patientPrescriptionImages = patientPrescriptionImages;
window.handlePatientFileSelection = handlePatientFileSelection;
window.removePatientPrescriptionImage = removePatientPrescriptionImage;
window.renderPatientPrescriptionGallery = renderPatientPrescriptionGallery;
window.updateWaitingRoomPrescriptionGallery = updateWaitingRoomPrescriptionGallery;
window.openMultiPagePrescriptionViewer = openMultiPagePrescriptionViewer;
window.switchPrescriptionViewerPage = switchPrescriptionViewerPage;
`;

const fnSwitchPage = `function switchPrescriptionViewerPage(delta) {
  const newIndex = currentActivePrescriptionPageIndex + delta;
  if (newIndex >= 0 && newIndex < patientPrescriptionImages.length) {
    openMultiPagePrescriptionViewer(newIndex);
  }
}`;

if (html.includes(fnSwitchPage) && !html.includes('// EXPORT MULTI-PAGE RX TO WINDOW')) {
  html = html.replace(fnSwitchPage, fnSwitchPage + '\n// EXPORT MULTI-PAGE RX TO WINDOW' + attachBlock);
  console.log("✓ Attached multi-page rx functions to window in Script 1");
}

// Update the init calls at bottom of Script 2 to be safe
const oldInit = `// Initialize multi-page prescription gallery on load
      try {
        if (typeof renderPatientPrescriptionGallery === 'function') renderPatientPrescriptionGallery();
        if (typeof updateWaitingRoomPrescriptionGallery === 'function') updateWaitingRoomPrescriptionGallery();
      } catch (e) {
        console.warn('Prescription gallery initial render deferred:', e);
      }`;

const safeInit = `// Initialize multi-page prescription gallery on load
      try {
        if (typeof window.renderPatientPrescriptionGallery === 'function') window.renderPatientPrescriptionGallery();
        else if (typeof renderPatientPrescriptionGallery === 'function') renderPatientPrescriptionGallery();
        if (typeof window.updateWaitingRoomPrescriptionGallery === 'function') window.updateWaitingRoomPrescriptionGallery();
        else if (typeof updateWaitingRoomPrescriptionGallery === 'function') updateWaitingRoomPrescriptionGallery();
      } catch (e) {
        console.warn('Prescription gallery initial render deferred:', e);
      }`;

if (html.includes(oldInit)) {
  html = html.replace(oldInit, safeInit);
  console.log("✓ Made init calls window-safe in Script 2");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully!");
