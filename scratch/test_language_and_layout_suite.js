const fs = require('fs');
const assert = require('assert');

console.log("=== RUNNING SWAHILI LANGUAGE & EMERGENCY CONTACT TEST SUITE ===");

const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');

// 1. Verify Emergency Contact position relative to Core Services and Upcoming Consultation inside #view-0
const view0Start = html.indexOf('id="view-0"');
const view0End = html.indexOf('id="view-1"', view0Start);
const view0Content = html.substring(view0Start, view0End);

const coreServicesIdx = view0Content.indexOf('core-services-2x2');
const helplineIdx = view0Content.indexOf('id="patientEmergencyHelplineStrip"');
const upcomingIdx = view0Content.indexOf('class="section-title">Upcoming Consultation');

assert(coreServicesIdx !== -1, "Core Services grid not found in view-0");
assert(helplineIdx !== -1, "Helpline strip #patientEmergencyHelplineStrip not found in view-0");
assert(upcomingIdx !== -1, "Upcoming Consultation section not found in view-0");

assert(helplineIdx > coreServicesIdx, "Emergency contact button must be placed AFTER Core Services in view-0");
assert(helplineIdx < upcomingIdx, "Emergency contact button must be placed BEFORE (above) Upcoming Consultation section in view-0");

// Verify Emergency Contact wording (MUST NOT be labelled 24/7)
assert(view0Content.includes('Emergency Contact'), "Helpline strip must be labelled 'Emergency Contact'");
assert(!view0Content.includes('24/7 Emergency Medical Helpline'), "Helpline strip must NOT be labelled '24/7'");
console.log("✓ Layout order & title verified: Core Services -> Emergency Contact (Call 16263) -> Upcoming Consultation");

// 2. Verify Medication Reminder card is REMOVED from Home View Upcoming Consultation
assert(!view0Content.includes('Azithromycin 500mg'), "Medication reminder card must be removed from home screen");
assert(!view0Content.includes('Scheduled 5:00 PM • Day 3 of 5'), "Medication reminder schedule must be removed from home screen");
assert(view0Content.includes('Dr. Sabrina Akter'), "Upcoming consultation with Dr. Sabrina Akter must remain intact");
console.log("✓ Verified: Medication Reminder card is 100% removed from Upcoming Consultation section");

// 3. Verify Language Menu & Modal Elements for Swahili
const requiredElements = [
  'patientLangMenuItem',
  'patientLangMenuText',
  'patientActiveLangBadge',
  'languageModal',
  'langOptEnglish',
  'langOptSwahili',
  'langCheckEnglish',
  'langCheckSwahili'
];

requiredElements.forEach(id => {
  assert(html.includes(`id="${id}"`), `Missing required DOM element: #${id}`);
  console.log(`✓ DOM element verified: #${id}`);
});

assert(html.includes('Language (English / Swahili)'), "Account menu must display Language (English / Swahili)");
assert(html.includes('Swahili (Kiswahili)'), "Language modal must have Swahili (Kiswahili) option");
assert(!html.includes('Language (English / বাংলা)'), "Old বাংলা language text must be removed from menu");
console.log("✓ Verified: Language menu & modal updated to English & Swahili");

// 4. Runtime Simulation of Language Switcher in Script 1
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
  appendChild(child) { this.children.push(child); return child; }
  removeChild(child) { return child; }
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
  createElement: (tag) => new MockElement('', tag),
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
eval(scripts[0]);

// Test openLanguageModal
assert(typeof global.openLanguageModal === 'function', "openLanguageModal must be a function");
global.openLanguageModal();
assert.strictEqual(getOrCreate('languageModal').style.display, 'flex', "languageModal should open with display flex");
console.log("✓ openLanguageModal() opened modal successfully");

// Test selecting Swahili language ('sw')
global.selectAppLanguage('sw');
assert.strictEqual(global.currentAppLanguage, 'sw', "Language state should be 'sw'");
assert.strictEqual(getOrCreate('langCheckSwahili').style.display, 'block', "Swahili checkmark should be visible");
assert.strictEqual(getOrCreate('langCheckEnglish').style.display, 'none', "English checkmark should be hidden");
console.log("✓ selectAppLanguage('sw') selected Swahili language");

// Test confirming Swahili language
global.confirmAppLanguage();
assert.strictEqual(getOrCreate('languageModal').style.display, 'none', "Modal should close on confirm");
assert.strictEqual(getOrCreate('patientActiveLangBadge').textContent, 'Swahili (Kiswahili)', "Active badge should update to Swahili");
assert.strictEqual(getOrCreate('patientLangMenuText').textContent, 'Language (English / Swahili)', "Menu text should update");
console.log("✓ confirmAppLanguage() successfully applied Swahili language & updated badge");

// Test switching back to English
global.openLanguageModal();
global.selectAppLanguage('en');
global.confirmAppLanguage();
assert.strictEqual(getOrCreate('patientActiveLangBadge').textContent, 'English', "Active badge should revert to English");
assert.strictEqual(getOrCreate('patientLangMenuText').textContent, 'Language (English / Swahili)', "Menu text should revert to English / Swahili");
console.log("✓ Switched back to English successfully");

console.log("\n============================================================");
console.log("🎉 ALL SWAHILI LANGUAGE & EMERGENCY CONTACT CHECKS PASSED 100%!");
console.log("============================================================");
