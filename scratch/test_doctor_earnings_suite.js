const fs = require('fs');
const assert = require('assert');

console.log("=== RUNNING DOCTOR EARNINGS & 20% DEBARRED PLATFORM FEE TEST SUITE ===");

const html = fs.readFileSync('prototype/index.html', 'utf8');

// 1. Verify DOM Elements
const requiredIds = [
  'doc-view-3',
  'docEarningsHeroCard',
  'docEarningsGrossRow',
  'docEarningsWithheldRow',
  'docEarningsFinalRow',
  'docGrossEarningsVal',
  'docWithheldFeeVal',
  'docFinalEarningsVal',
  'docEarningsCalcNote',
  'docConsultationsLedgerContainer',
  'doctorStatementModal',
  'dsmDocTitle',
  'dsmGrossVal',
  'dsmWithheldVal',
  'dsmFinalVal',
  'dsmItemizedTbody',
  'docConsultationFeeModal',
  'dcfmPatientTitle',
  'dcfmSessionSub',
  'dcfmTrxId',
  'dcfmGross',
  'dcfmWithheld',
  'dcfmNet'
];

requiredIds.forEach(id => {
  assert(html.includes(`id="${id}"`), `Missing required DOM ID: #${id}`);
  console.log(`✓ DOM element verified: #${id}`);
});

// 2. Check for the user-required explanatory text
const requiredPhrase = "total calculation is based including the platform charge 20%";
const normalizedHtml = html.toLowerCase();
assert(normalizedHtml.includes(requiredPhrase), `HTML must contain the exact requirement: "${requiredPhrase}"`);
console.log(`✓ Explanatory text verified: "${requiredPhrase}"`);

// 3. Verify Doctor Web Shell card has 20% debarment breakdown
assert(html.includes('Total Gross: <b>৳ 10,500</b>'), "Doctor Web Shell missing Gross revenue");
assert(html.includes('20% Fee: <b>- ৳ 2,100</b>'), "Doctor Web Shell missing 20% Fee");
assert(html.includes('৳ 8,400 <span style="font-size:11px; font-weight:600; color:#059669;">Net Final</span>'), "Doctor Web Shell missing Net Final");
console.log("✓ Doctor Web Shell revenue card verified with 20% debarment");

// 4. Runtime Simulation of Script Block 1
class MockClassList {
  constructor() { this.classes = new Set(); }
  add(...c) { c.forEach(x => this.classes.add(x)); }
  remove(...c) { c.forEach(x => this.classes.delete(x)); }
  toggle(c, force) {
    if (force === undefined) {
      if (this.classes.has(c)) this.classes.delete(c);
      else this.classes.add(c);
    } else if (force) {
      this.classes.add(c);
    } else {
      this.classes.delete(c);
    }
  }
  contains(c) { return this.classes.has(c); }
}

class MockElement {
  constructor(id = '', tag = 'div') {
    this.id = id;
    this.tagName = tag.toUpperCase();
    this.style = {};
    this.classList = new MockClassList();
    this.innerHTML = '';
    this.textContent = '';
    this.children = [];
    this.parentElement = null;
    this.attributes = {};
  }
  getAttribute(name) { return this.attributes[name] || ''; }
  setAttribute(name, val) { this.attributes[name] = val; }
  querySelector() { return null; }
  querySelectorAll() { return []; }
  addEventListener() {}
  removeEventListener() {}
}

const elements = {};
function getOrCreate(id) {
  if (!elements[id]) elements[id] = new MockElement(id);
  return elements[id];
}

const mockDocument = {
  getElementById: (id) => getOrCreate(id),
  querySelector: () => new MockElement(),
  querySelectorAll: () => [],
  addEventListener: () => {},
  body: new MockElement('body', 'body')
};

global.document = mockDocument;
global.window = global;
global.addEventListener = () => {};
global.removeEventListener = () => {};
global.showAppToast = (title, msg) => console.log(`[TOAST]: ${title} -> ${msg}`);

const scripts = [...html.matchAll(/<script[\s\S]*?>([\s\S]*?)<\/script>/gi)].map(m => m[1]);
eval(scripts[0]); // Evaluate Patient/Doctor Script Block 1

// Verify doctorEarningsStore
assert(typeof global.doctorEarningsStore === 'object', "doctorEarningsStore must exist");
const store = global.doctorEarningsStore;
console.log("\nVerifying Financial Math in doctorEarningsStore:");
console.log(`- Doctor:             ${store.doctorName} (${store.bmdc})`);
console.log(`- Gross Earnings:     ৳ ${store.totalGross}`);
console.log(`- Platform Charge:    ${store.platformChargePercent}%`);
console.log(`- Withheld Fee (20%): - ৳ ${store.withheldFee}`);
console.log(`- Final Net Earning:  ৳ ${store.finalNet}`);

assert.strictEqual(store.totalGross * 0.20, store.withheldFee, "Withheld fee must be exactly 20% of gross");
assert.strictEqual(store.totalGross - store.withheldFee, store.finalNet, "Final net must equal gross minus 20% withheld");

store.consultations.forEach(c => {
  assert.strictEqual(c.gross * 0.20, c.withheld, `Withheld fee mismatch for ${c.patient}`);
  assert.strictEqual(c.gross - c.withheld, c.net, `Net earning mismatch for ${c.patient}`);
  console.log(`  ✓ ${c.patient}: ৳ ${c.gross} (Total) - ৳ ${c.withheld} (20%) = ৳ ${c.net} (Final Net) [${c.status}]`);
});

// Test renderDoctorEarnings
global.renderDoctorEarnings();
assert(getOrCreate('docGrossEarningsVal').textContent.includes('35,562.50'), "docGrossEarningsVal not updated");
assert(getOrCreate('docWithheldFeeVal').innerHTML.includes('7,112.50'), "docWithheldFeeVal not updated");
assert(getOrCreate('docFinalEarningsVal').textContent.includes('28,450.00'), "docFinalEarningsVal not updated");
console.log("✓ renderDoctorEarnings() populated all 3-tier cards correctly");

// Test openDoctorStatementModal
global.openDoctorStatementModal();
assert.strictEqual(getOrCreate('doctorStatementModal').style.display, 'flex', "doctorStatementModal should open");
assert(getOrCreate('dsmGrossVal').textContent.includes('35,562.50'), "dsmGrossVal not populated");
assert(getOrCreate('dsmWithheldVal').textContent.includes('7,112.50'), "dsmWithheldVal not populated");
assert(getOrCreate('dsmFinalVal').textContent.includes('28,450.00'), "dsmFinalVal not populated");
console.log("✓ openDoctorStatementModal() opened with populated statement table");

global.closeDoctorStatementModal();
assert.strictEqual(getOrCreate('doctorStatementModal').style.display, 'none', "doctorStatementModal should close");
console.log("✓ closeDoctorStatementModal() closed modal");

// Test openConsultationFeeDetailModal
global.openConsultationFeeDetailModal('Rafiq Ahmed', 'Video Consultation (10 min)', 800, 160, 640, 'BK-MER-7718290');
assert.strictEqual(getOrCreate('docConsultationFeeModal').style.display, 'flex', "docConsultationFeeModal should open");
assert.strictEqual(getOrCreate('dcfmPatientTitle').textContent, 'Fee Breakdown: Rafiq Ahmed');
assert.strictEqual(getOrCreate('dcfmGross').textContent, '৳ 800');
assert(getOrCreate('dcfmWithheld').innerHTML.includes('160'));
assert.strictEqual(getOrCreate('dcfmNet').textContent, '৳ 640');
console.log("✓ openConsultationFeeDetailModal() rendered individual consultation 20% breakdown");

global.closeConsultationFeeModal();
assert.strictEqual(getOrCreate('docConsultationFeeModal').style.display, 'none', "docConsultationFeeModal should close");
console.log("✓ closeConsultationFeeModal() closed modal");

// Test switchDoctorTab(3) triggers renderDoctorEarnings
global.switchDoctorTab(3);
console.log("✓ switchDoctorTab(3) executed successfully");

console.log("\n============================================================");
console.log("🎉 ALL DOCTOR EARNINGS & 20% DEBARMENT CHECKS PASSED 100%!");
console.log("============================================================");
