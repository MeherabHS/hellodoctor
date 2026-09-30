const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

const targetStr = 'window.reconcileAdminMfsWebhooks = reconcileAdminMfsWebhooks;';
const initCall = `window.reconcileAdminMfsWebhooks = reconcileAdminMfsWebhooks;

      // Initialize multi-page prescription gallery on load
      try {
        if (typeof renderPatientPrescriptionGallery === 'function') renderPatientPrescriptionGallery();
        if (typeof updateWaitingRoomPrescriptionGallery === 'function') updateWaitingRoomPrescriptionGallery();
      } catch (e) {
        console.warn('Prescription gallery initial render deferred:', e);
      }`;

if (html.includes(targetStr) && !html.includes('Prescription gallery initial render deferred:')) {
  html = html.replace(targetStr, initCall);
  fs.writeFileSync(targetFile, html, 'utf8');
  console.log("✓ Added initial render call for prescription galleries");
} else {
  console.log("Already added or not matched");
}
