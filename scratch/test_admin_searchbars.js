const fs = require('fs');
const assert = require('assert');
const vm = require('vm');

console.log('>>> [1/4] Checking HTML Tag Balance...');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const checkTags = ['main', 'script', 'body', 'html'];
checkTags.forEach(tag => {
  const opens = (html.match(new RegExp(`<${tag}[\\s>]`, 'gi')) || []).length;
  const closes = (html.match(new RegExp(`</${tag}>`, 'gi')) || []).length;
  console.log(`- <${tag}>: open=${opens}, close=${closes} -> ${opens === closes ? 'OK' : 'MISMATCH!'}`);
  assert.strictEqual(opens, closes, `<${tag}> tags must be balanced`);
});

const openDivs = (html.match(/<div(\s|>)/gi) || []).length;
const closeDivs = (html.match(/<\/div>/gi) || []).length;
console.log(`- <div>: open=${openDivs}, close=${closeDivs} -> ${openDivs === closeDivs ? 'OK' : 'MISMATCH!'}`);
assert.strictEqual(openDivs, closeDivs, 'Div tags must be balanced');

console.log('\n>>> [2/4] Parsing inline scripts for syntax errors...');
const scriptBlocks = [...html.matchAll(/<script[\s\S]*?>([\s\S]*?)<\/script>/gi)].map(m => m[1]);
assert.ok(scriptBlocks.length >= 2, 'Should find at least 2 script blocks');

scriptBlocks.forEach((code, idx) => {
  try {
    new vm.Script(code);
    console.log(`✓ Script block #${idx + 1} (${code.length} chars) parsed successfully with 0 syntax errors!`);
  } catch (err) {
    console.error(`SCRIPT BLOCK #${idx + 1} SYNTAX ERROR:`, err.message);
    process.exit(1);
  }
});

console.log('\n>>> [3/4] Verifying required DOM elements for Doctor & Patient Searchbars...');
const requiredIds = [
  'adminDoctorSearchInput',
  'adminPatientSearchInput',
  'adminDoctorsTableBody',
  'adminPatientsTableBody'
];

requiredIds.forEach(id => {
  assert.ok(html.includes(`id="${id}"`), `DOM element id="${id}" must exist in HTML`);
});
console.log(`All ${requiredIds.length} required DOM IDs for Doctor & Patient Searchbars verified!`);

console.log('\n>>> [4/4] Verifying Function Definitions in script...');
const allScript = scriptBlocks.join('\n');
const requiredFunctions = [
  'filterAdminDoctorsTable',
  'filterAdminDoctorsStatus',
  'applyAdminDoctorsFilters',
  'handleAdminPatientSearch',
  'filterAdminPatients',
  'applyAdminPatientsFilters',
  'handleAdminGlobalSearch'
];

requiredFunctions.forEach(fn => {
  assert.ok(allScript.includes(`function ${fn}`), `Function ${fn} must be defined`);
});
console.log(`All ${requiredFunctions.length} Doctor & Patient search controller functions verified!`);

console.log('\n✅ ALL DOCTOR & PATIENT SEARCHBAR CHECKS PASSED WITH 100% SUCCESS!');
