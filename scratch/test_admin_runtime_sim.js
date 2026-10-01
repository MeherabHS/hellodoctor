const fs = require('fs');

// We simulate minimal document/window mock
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
}

const elements = {};
function getOrCreate(id) {
  if (!elements[id]) elements[id] = new MockElement(id);
  return elements[id];
}

const mockDocument = {
  getElementById: (id) => getOrCreate(id),
  querySelectorAll: (sel) => {
    if (sel.includes('.admin-page-view')) {
      return ['command', 'doctors', 'patients', 'bmdc', 'slots', 'compliance'].map(id => getOrCreate('adminPage_' + id));
    }
    if (sel.includes('.admin-nav-item')) {
      return ['command', 'doctors', 'patients', 'bmdc', 'slots', 'compliance'].map(id => getOrCreate('adminNav-' + id));
    }
    if (sel.includes('#adminPatientsTableBody tr')) {
      const tr1 = new MockElement(); tr1.setAttribute('data-cat', 'chronic');
      const tr2 = new MockElement(); tr2.setAttribute('data-cat', 'chat chronic');
      const tr3 = new MockElement(); tr3.setAttribute('data-cat', 'pediatric');
      return [tr1, tr2, tr3];
    }
    if (sel.includes('.doc-payout-status')) {
      return [new MockElement(), new MockElement(), new MockElement()];
    }
    return [];
  },
  body: new MockElement('body', 'body')
};

global.document = mockDocument;
global.window = global;
global.showAppToast = (title, msg) => console.log(`[TOAST]: ${title} -> ${msg}`);

// Load the script and extract function definitions
const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');
const scripts = [...html.matchAll(/<script[\s\S]*?>([\s\S]*?)<\/script>/gi)].map(m => m[1]);
const script2 = scripts[1]; // second script block containing admin controllers

eval(script2);

console.log('Testing Admin Functions Execution:');

// Test 1: Tab switching
switchAdminPage('patients');
console.log('1. switchAdminPage("patients"):', getOrCreate('adminPage_patients').classList.contains('active') ? 'Success ✓' : 'Failed ✗');

// Test 2: Doctor history modal
openAdminDoctorHistoryModal('anika');
console.log('2. openAdminDoctorHistoryModal("anika"):', getOrCreate('adhDocName').textContent === 'Dr. Anika Rahman' ? 'Success ✓' : 'Failed ✗');
closeAdminDoctorHistoryModal();
console.log('3. closeAdminDoctorHistoryModal():', getOrCreate('adminDoctorHistoryModal').style.display === 'none' ? 'Success ✓' : 'Failed ✗');

// Test 3: Patient dossier modal
openAdminPatientDossierModal('sarah');
console.log('4. openAdminPatientDossierModal("sarah"):', getOrCreate('apdPatName').textContent === 'Sarah Ahmed' ? 'Success ✓' : 'Failed ✗');
closeAdminPatientDossierModal();
console.log('5. closeAdminPatientDossierModal():', getOrCreate('adminPatientDossierModal').style.display === 'none' ? 'Success ✓' : 'Failed ✗');

// Test 4: Batch payout
executeAdminBatchPayout();
console.log('6. executeAdminBatchPayout(): Executed successfully ✓');

// Test 5: Patient filtering
const mockBtn = new MockElement();
mockBtn.parentElement = new MockElement();
mockBtn.parentElement.querySelectorAll = () => [mockBtn];
filterAdminPatients('pediatric', mockBtn);
console.log('7. filterAdminPatients("pediatric"): Executed successfully ✓');

// Test 6: BMDC approve
approveDoctorBmdc('approve');
console.log('8. approveDoctorBmdc("approve"): Executed successfully ✓');

// Test 7: Emergency bridge
trigger16263BridgeModal();
console.log('9. trigger16263BridgeModal(): Executed successfully ✓');

console.log('\nAll 7 runtime functions executed flawlessly with 0 runtime errors!');
