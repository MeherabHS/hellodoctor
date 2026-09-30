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

console.log('\n>>> [3/4] Verifying required DOM elements for Error Logs section...');
const requiredIds = [
  'adminNav-logs',
  'adminLogsNavBadge',
  'adminPage_logs',
  'adminLogSearchInput',
  'adminLogSeverityFilter',
  'adminLogSubsystemFilter',
  'adminLogStatusFilter',
  'adminLogsTableBody',
  'adminLogsVisibleCount',
  'adminKpiTotalLogs',
  'adminKpiImpactedUsers',
  'adminLogDetailModal',
  'aldTraceId',
  'aldUserName',
  'aldSubsystem',
  'aldComponent',
  'aldActionAttempted',
  'aldErrorMessage',
  'aldStackTrace',
  'adminSimulateFailureModal',
  'simFailUserSelect',
  'simFailScenarioSelect',
  'simFailSeverity',
  'simFailCustomNote'
];

requiredIds.forEach(id => {
  assert.ok(html.includes(`id="${id}"`), `DOM element id="${id}" must exist in HTML`);
});
console.log(`All ${requiredIds.length} required DOM IDs for Error Logs verified!`);

console.log('\n>>> [4/4] Verifying Data Store & Function Definitions in script...');
const allScript = scriptBlocks.join('\n');
const requiredFunctions = [
  'renderAdminLogsTable',
  'getFilteredAdminLogs',
  'filterAdminLogs',
  'setAdminLogQuickFilter',
  'updateAdminLogsTelemetryCounts',
  'openAdminLogDetail',
  'closeAdminLogDetail',
  'toggleAdminLogResolveStatus',
  'toggleAdminLogRowResolve',
  'retryAdminLogAction',
  'refundAdminLogUser',
  'copyAdminLogTrace',
  'openAdminSimulateFailureModal',
  'closeAdminSimulateFailureModal',
  'onSimFailScenarioChange',
  'executeSimulateFailure',
  'logAppIncident',
  'exportAdminLogs',
  'refreshAdminLogs'
];

requiredFunctions.forEach(fn => {
  assert.ok(allScript.includes(`function ${fn}`), `Function ${fn} must be defined`);
});
console.log(`All ${requiredFunctions.length} Error Log controller functions verified!`);

// Verify initial log store contents
assert.ok(allScript.includes('adminAppErrorLogsStore = ['), 'adminAppErrorLogsStore must be defined');
assert.ok(allScript.includes('TRC-94812-BKASH'), 'bKash log trace must be present');
assert.ok(allScript.includes('TRC-94811-WEBRTC'), 'WebRTC log trace must be present');
assert.ok(allScript.includes('TRC-94788-SYNC'), 'Offline sync log trace must be present');
assert.ok(allScript.includes('TRC-94765-DGDA'), 'DGDA prescription log trace must be present');

console.log('Incident store verified with bKash, WebRTC, Sync, and DGDA traces!');

console.log('\n✅ ALL ADMIN ERROR LOGS TESTS PASSED WITH 100% SUCCESS!');
