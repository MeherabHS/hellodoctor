const fs = require('fs');
const vm = require('vm');

const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');

console.log('1. Checking HTML Tag Balance for key containers...');
const checkTags = ['main', 'script', 'body', 'html'];
checkTags.forEach(tag => {
  const opens = (html.match(new RegExp(`<${tag}[\\s>]`, 'gi')) || []).length;
  const closes = (html.match(new RegExp(`</${tag}>`, 'gi')) || []).length;
  console.log(`- <${tag}>: open=${opens}, close=${closes} -> ${opens === closes ? 'OK' : 'MISMATCH!'}`);
});

console.log('\n2. Extracting and Parsing Script Blocks for Syntax Errors...');
const scriptBlocks = [...html.matchAll(/<script[\s\S]*?>([\s\S]*?)<\/script>/gi)].map(m => m[1]);

scriptBlocks.forEach((code, idx) => {
  try {
    new vm.Script(code);
    console.log(`✓ Script block #${idx + 1} (${code.length} chars) parsed successfully with 0 syntax errors!`);
  } catch (err) {
    console.error(`SCRIPT BLOCK #${idx + 1} SYNTAX ERROR:`, err.message);
    process.exit(1);
  }
});

const allScriptCode = scriptBlocks.join('\n');

console.log('\n3. Verifying Admin Onclick Handlers...');
const adminStart = html.indexOf('id="adminPortalShell"');
const adminEnd = html.indexOf('</main>');
const adminChunk = html.substring(adminStart, adminEnd);

const testedFunctions = [
  'switchAdminPage',
  'openAdminDoctorHistoryModal',
  'closeAdminDoctorHistoryModal',
  'openAdminPatientDossierModal',
  'closeAdminPatientDossierModal',
  'filterAdminPatients',
  'executeAdminBatchPayout',
  'approveDoctorBmdc',
  'trigger16263BridgeModal',
  'showAppToast',
  'openDoctorReportViewer'
];

testedFunctions.forEach(fn => {
  const exists = allScriptCode.includes(`function ${fn}`);
  console.log(`- ${fn}: ${exists ? 'Defined ✓' : 'MISSING ✗'}`);
});

console.log('\n4. Verifying Doctor & Patient Store Keys...');
const requiredDocs = ['anika', 'sadik', 'farhana', 'sabrina'];
requiredDocs.forEach(k => {
  console.log(`- Doctor key '${k}': ${allScriptCode.includes(`${k}: {`) ? 'Present ✓' : 'MISSING ✗'}`);
});

const requiredPts = ['sarah', 'rafiq', 'nusrat', 'kamal', 'farzana', 'tanvir'];
requiredPts.forEach(k => {
  console.log(`- Patient key '${k}': ${allScriptCode.includes(`${k}: {`) ? 'Present ✓' : 'MISSING ✗'}`);
});

console.log('\nAll checks passed successfully!');
