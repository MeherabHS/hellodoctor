const fs = require('fs');
const assert = require('assert');

console.log("=== RUNNING DOCTOR DIRECTORY & DOSSIER MODAL TEST SUITE ===");

const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');

// 1. Verify DOM Elements in modal
assert(html.includes('id="adhDocPhone"'), "Missing #adhDocPhone in modal");
assert(html.includes('id="adhDocResidence"'), "Missing #adhDocResidence in modal");
assert(html.includes('id="adhDocEmail"'), "Missing #adhDocEmail in modal");
console.log("✓ DOM check passed: #adhDocPhone, #adhDocResidence, #adhDocEmail exist");

// 2. Verify Doctor Directory Table rows have phone and residence
const docRows = ['anika', 'sadik', 'farhana', 'sabrina'];
const expectedPhones = {
  anika: '+880 1711-884920',
  sadik: '+880 1819-334455',
  farhana: '+880 1912-778899',
  sabrina: '+880 1713-445566'
};
const expectedResidences = {
  anika: 'Dhanmondi, Dhaka',
  sadik: 'Gulshan-2, Dhaka',
  farhana: 'Uttara, Dhaka',
  sabrina: 'Banani, Dhaka'
};

for (const key of docRows) {
  assert(html.includes(expectedPhones[key]), `Missing phone for ${key}: ${expectedPhones[key]}`);
  assert(html.includes(expectedResidences[key]), `Missing residence for ${key}: ${expectedResidences[key]}`);
}
console.log("✓ Directory table check passed: all 4 doctor rows have contact & residence info");

// 3. Runtime Simulation
class MockClassList {
  constructor() { this.classes = new Set(); }
  add(...c) { c.forEach(x => this.classes.add(x)); }
  remove(...c) { c.forEach(x => this.classes.delete(x)); }
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
  querySelectorAll: () => [],
  addEventListener: () => {},
  body: new MockElement('body', 'body')
};

global.document = mockDocument;
global.window = global;
global.showAppToast = () => {};

const scripts = [...html.matchAll(/<script[\s\S]*?>([\s\S]*?)<\/script>/gi)].map(m => m[1]);
// We append window exports for testing access
eval(scripts[1] + '\n;global.adminDoctorHistoryStore = adminDoctorHistoryStore; global.openAdminDoctorHistoryModal = openAdminDoctorHistoryModal;');

assert(typeof global.openAdminDoctorHistoryModal === 'function', "openAdminDoctorHistoryModal should be a function");
assert(typeof global.adminDoctorHistoryStore === 'object', "adminDoctorHistoryStore should be an object");

for (const key of docRows) {
  openAdminDoctorHistoryModal(key);
  const doc = adminDoctorHistoryStore[key];
  
  assert.strictEqual(getOrCreate('adhDocName').textContent, doc.name, `Name mismatch for ${key}`);
  assert.strictEqual(getOrCreate('adhDocBmdc').textContent, doc.bmdc, `BMDC mismatch for ${key}`);
  assert.strictEqual(getOrCreate('adhDocPhone').textContent, doc.phone, `Phone mismatch for ${key}`);
  assert.strictEqual(getOrCreate('adhDocResidence').textContent, doc.residence, `Residence mismatch for ${key}`);
  assert.strictEqual(getOrCreate('adhDocEmail').textContent, doc.email, `Email mismatch for ${key}`);
  assert.strictEqual(getOrCreate('adminDoctorHistoryModal').style.display, 'flex', "Modal should be open");
  console.log(`✓ Dossier Modal verified for ${doc.name}:`);
  console.log(`    - Phone:     ${doc.phone}`);
  console.log(`    - Residence: ${doc.residence}`);
  console.log(`    - Email:     ${doc.email}`);
}

console.log("\n=== ALL DOCTOR DIRECTORY & DOSSIER TESTS PASSED SUCCESSFULLY! ===");
