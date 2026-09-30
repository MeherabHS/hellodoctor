const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

// 1. Hide #pgmTelemetryInspector on patient end
const oldPgmTelem = '<div style="background:#F0FDF4; border:1.5px solid #86EFAC; border-radius:12px; padding:12px 14px; margin-bottom:14px;" id="pgmTelemetryInspector">';
const newPgmTelem = '<div style="display:none;" id="pgmTelemetryInspector" aria-hidden="true">';

if (html.includes(oldPgmTelem)) {
  html = html.replace(oldPgmTelem, newPgmTelem);
  console.log("✓ Hidden #pgmTelemetryInspector on patient end");
} else {
  console.log("Target #pgmTelemetryInspector string not found or already hidden");
}

// 2. Hide #v11TelemetryCard on patient end
const oldV11Telem = '<div style="margin-top:14px; background:#F8FAFC; border:1px solid #E2E8F0; border-radius:12px; padding:12px 14px;" id="v11TelemetryCard">';
const newV11Telem = '<div style="display:none;" id="v11TelemetryCard" aria-hidden="true">';

if (html.includes(oldV11Telem)) {
  html = html.replace(oldV11Telem, newV11Telem);
  console.log("✓ Hidden #v11TelemetryCard on patient end");
} else {
  console.log("Target #v11TelemetryCard string not found or already hidden");
}

// 3. Update the Administrative Governance Protocol text in the patient modal to reassure the patient without showing raw telemetry
const oldGovProto = 'Patients submit factual reports of clinical or technical failures. All dispute remedies (financial refunds, technical fixes, or professional conduct reviews) are adjudicated independently by the Central Medical Directorate under BMDC standards.';
const newGovProto = 'Patients submit factual reports of clinical or technical failures. All dispute remedies (financial refunds, technical fixes, or professional conduct reviews) are adjudicated independently by the Central Medical Directorate under BMDC standards. Consultation logs and session timestamps are automatically verified in the background by Medical Governance.';

if (html.includes(oldGovProto) && !html.includes(newGovProto)) {
  html = html.replace(oldGovProto, newGovProto);
  console.log("✓ Updated governance protocol copy to reassure patient");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully!");
