const fs = require('fs');
const assert = require('assert');

console.log('>>> [1/4] Checking file reading and HTML tag balance...');
const html = fs.readFileSync('prototype/index.html', 'utf8');

// Basic tag check
const openDivs = (html.match(/<div(\s|>)/gi) || []).length;
const closeDivs = (html.match(/<\/div>/gi) || []).length;
console.log(`Div tags: open=${openDivs}, close=${closeDivs}`);
assert.strictEqual(openDivs, closeDivs, 'Div tags must be balanced');

// Check script syntax
console.log('>>> [2/4] Parsing inline scripts for syntax errors...');
const scriptMatches = html.match(/<script[\s\S]*?<\/script>/gi);
assert.ok(scriptMatches && scriptMatches.length >= 2, 'Should find at least 2 script tags');

scriptMatches.forEach((sc, i) => {
  const code = sc.replace(/<\/?script[^>]*>/gi, '');
  try {
    new Function(code);
    console.log(`Script block ${i + 1} passed syntax check!`);
  } catch (err) {
    console.error(`Syntax error in script block ${i + 1}:`, err);
    process.exit(1);
  }
});

// Check DOM Elements existence
console.log('>>> [3/4] Verifying required DOM element IDs...');
const requiredIds = [
  'view-14',                    // Screen 14 skeleton loading
  'wbNetworkToggle',            // Workbench network toggle button
  'wbNetworkLabel',             // Network button label
  'wbLoadingSimBtn',            // Workbench simulate loading button
  'patientOfflineBanner',       // Patient app offline banner
  'doctorOfflineBanner',        // Doctor mobile offline banner
  'docWebOfflineBanner',        // Doctor web offline banner
  'adminOfflineBanner',         // Central admin offline banner
  'offlineStatusModal',         // Offline status modal
  'offlineActionGuardModal',    // Offline action guard modal
  'offlineGuardActionTitle',    // Action guard title element
  'onlineRecoveryToast',        // Online recovery toast
  'globalLoadingOverlay',       // Global loading overlay
  'globalLoadingTitle',         // Loading overlay title
  'globalLoadingSubtitle'       // Loading overlay subtitle
];

requiredIds.forEach(id => {
  assert.ok(html.includes(`id="${id}"`), `Element id="${id}" must exist in HTML`);
});
console.log(`All ${requiredIds.length} required DOM IDs verified successfully!`);

// Verify CSS classes
console.log('>>> [4/4] Verifying CSS classes and keyframes...');
const requiredStrings = [
  '@keyframes skShimmer',
  '.sk-shimmer',
  '.offline-banner',
  'body.is-offline .offline-banner',
  '.global-loading-overlay',
  '.online-recovery-toast'
];

requiredStrings.forEach(s => {
  assert.ok(html.includes(s), `CSS should include "${s}"`);
});
console.log('All CSS rules and keyframes verified!');

console.log('\n✅ ALL TEST CHECKS PASSED PERFECTLY!');
