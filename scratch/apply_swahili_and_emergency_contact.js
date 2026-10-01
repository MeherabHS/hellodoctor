const fs = require('fs');

const targetFile = 'index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original index.html file size:", html.length);

// 1. Update Emergency Helpline banner: rename from "24/7 Emergency Medical Helpline" to "Emergency Contact"
const oldHelplineBanner = `<div id="patientEmergencyHelplineStrip" style="margin:14px 18px 6px; background:linear-gradient(135deg, #064E3B 0%, #047857 100%); border-radius:14px; padding:10px 14px; display:flex; justify-content:space-between; align-items:center; color:#fff; box-shadow:0 3px 10px rgba(5,150,105,0.2);">
              <div style="display:flex; align-items:center; gap:9px;">
                <div style="width:30px; height:30px; border-radius:50%; background:rgba(255,255,255,0.22); display:grid; place-items:center; font-size:14px;">📞</div>
                <div>
                  <div style="font-size:11.5px; font-weight:800; letter-spacing:0.2px;">24/7 Emergency Medical Helpline</div>
                  <div style="font-size:9.5px; opacity:0.92;">Emergency triage &amp; rapid physician assistance</div>
                </div>
              </div>
              <a href="tel:16263" style="background:#fff; color:#064E3B; font-size:11px; font-weight:800; padding:5px 10px; border-radius:8px; text-decoration:none; display:inline-flex; align-items:center; gap:4px; box-shadow:0 1px 3px rgba(0,0,0,0.1);" onclick="event.stopPropagation(); showAppToast('Calling Helpline 📞', 'Connecting to 24/7 Emergency Medical Hotline: 16263', 'info', '📞');">
                <span>Call 16263</span>
              </a>
            </div>`;

const newHelplineBanner = `<div id="patientEmergencyHelplineStrip" style="margin:14px 18px 6px; background:linear-gradient(135deg, #064E3B 0%, #047857 100%); border-radius:14px; padding:10px 14px; display:flex; justify-content:space-between; align-items:center; color:#fff; box-shadow:0 3px 10px rgba(5,150,105,0.2);">
              <div style="display:flex; align-items:center; gap:9px;">
                <div style="width:30px; height:30px; border-radius:50%; background:rgba(255,255,255,0.22); display:grid; place-items:center; font-size:14px;">📞</div>
                <div>
                  <div style="font-size:12px; font-weight:800; letter-spacing:0.2px;">Emergency Contact</div>
                  <div style="font-size:9.5px; opacity:0.92;">Emergency triage &amp; rapid physician assistance</div>
                </div>
              </div>
              <a href="tel:16263" style="background:#fff; color:#064E3B; font-size:11px; font-weight:800; padding:5px 10px; border-radius:8px; text-decoration:none; display:inline-flex; align-items:center; gap:4px; box-shadow:0 1px 3px rgba(0,0,0,0.1);" onclick="event.stopPropagation(); showAppToast('Calling Emergency Contact 📞', 'Connecting to Emergency Medical Helpline: 16263', 'info', '📞');">
                <span>Call 16263</span>
              </a>
            </div>`;

if (html.includes(oldHelplineBanner)) {
  html = html.replace(oldHelplineBanner, newHelplineBanner);
  console.log("✓ Updated Emergency banner to 'Emergency Contact' (removed 24/7)");
} else {
  console.error("✗ Could not find oldHelplineBanner");
}

// 2. Update language menu item: Language (English / Swahili)
const oldLangMenu = `<div class="menu-text" id="patientLangMenuText">Language (English / Kenyan)</div>`;
const newLangMenu = `<div class="menu-text" id="patientLangMenuText">Language (English / Swahili)</div>`;

if (html.includes(oldLangMenu)) {
  html = html.replace(oldLangMenu, newLangMenu);
  console.log("✓ Updated menu text to 'Language (English / Swahili)'");
} else {
  console.error("✗ Could not find oldLangMenu");
}

// 3. Update #languageModal options: Kenyan -> Swahili
const oldLangModalOptions = `              <!-- Option 2: Kenyan (Kiswahili / Kenyan English) -->
              <div class="lang-option-card" id="langOptKenyan" onclick="selectAppLanguage('ke')" style="display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-radius:14px; border:1.5px solid var(--border); background:var(--surface); cursor:pointer; transition:all 0.15s ease;">
                <div style="display:flex; align-items:center; gap:12px;">
                  <div style="font-size:22px; line-height:1;">🇰🇪</div>
                  <div>
                    <div style="font-size:13px; font-weight:800; color:var(--text-main);">Kenyan (Kiswahili / English)</div>
                    <div style="font-size:10.5px; color:var(--text-sub);">Lugha ya Kiswahili na Kiingereza cha Kenya</div>
                  </div>
                </div>
                <div class="lang-check" id="langCheckKenyan" style="color:var(--primary); font-weight:900; font-size:16px; display:none;">✓</div>
              </div>`;

const newLangModalOptions = `              <!-- Option 2: Swahili (Kiswahili) -->
              <div class="lang-option-card" id="langOptSwahili" onclick="selectAppLanguage('sw')" style="display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-radius:14px; border:1.5px solid var(--border); background:var(--surface); cursor:pointer; transition:all 0.15s ease;">
                <div style="display:flex; align-items:center; gap:12px;">
                  <div style="font-size:22px; line-height:1;">🇰🇪</div>
                  <div>
                    <div style="font-size:13px; font-weight:800; color:var(--text-main);">Swahili (Kiswahili)</div>
                    <div style="font-size:10.5px; color:var(--text-sub);">Lugha ya Kiswahili • Rasmi ya Kenya na Afrika Mashariki</div>
                  </div>
                </div>
                <div class="lang-check" id="langCheckSwahili" style="color:var(--primary); font-weight:900; font-size:16px; display:none;">✓</div>
              </div>`;

if (html.includes(oldLangModalOptions)) {
  html = html.replace(oldLangModalOptions, newLangModalOptions);
  console.log("✓ Updated #languageModal Option 2 to 'Swahili (Kiswahili)'");
} else {
  console.error("✗ Could not find oldLangModalOptions");
}

// 4. Update JavaScript language controllers
const oldJsLanguage = `/* ═════════════════════════════════════════════════════════════════════════════
   PATIENT APP LANGUAGE LOCALIZATION (ENGLISH & KENYAN)
   ═════════════════════════════════════════════════════════════════════════════ */
let currentAppLanguage = 'en';

function openLanguageModal() {
  updateLanguageModalUI();
  const modal = document.getElementById('languageModal');
  if (modal) modal.style.display = 'flex';
}

function closeLanguageModal() {
  const modal = document.getElementById('languageModal');
  if (modal) modal.style.display = 'none';
}

function selectAppLanguage(langKey) {
  currentAppLanguage = langKey;
  window.currentAppLanguage = langKey;
  updateLanguageModalUI();
}

function updateLanguageModalUI() {
  const isEn = currentAppLanguage === 'en';
  const optEn = document.getElementById('langOptEnglish');
  const optKe = document.getElementById('langOptKenyan');
  const checkEn = document.getElementById('langCheckEnglish');
  const checkKe = document.getElementById('langCheckKenyan');

  if (optEn) {
    optEn.style.borderColor = isEn ? 'var(--primary)' : 'var(--border)';
    optEn.style.background = isEn ? 'var(--primary-light)' : 'var(--surface)';
  }
  if (optKe) {
    optKe.style.borderColor = !isEn ? 'var(--primary)' : 'var(--border)';
    optKe.style.background = !isEn ? 'var(--primary-light)' : 'var(--surface)';
  }
  if (checkEn) checkEn.style.display = isEn ? 'block' : 'none';
  if (checkKe) checkKe.style.display = !isEn ? 'block' : 'none';
}

function confirmAppLanguage() {
  closeLanguageModal();
  window.currentAppLanguage = currentAppLanguage;
  const badge = document.getElementById('patientActiveLangBadge');
  const menuText = document.getElementById('patientLangMenuText');
  if (currentAppLanguage === 'ke') {
    if (badge) badge.textContent = 'Kenyan (Kiswahili)';
    if (menuText) menuText.textContent = 'Language (Kenyan / Kiswahili)';
    showAppToast('Lugha Imebadilishwa 🇰🇪', 'App language set to Kenyan (Kiswahili / English). Karibu HelloDoctor!', 'success', '🇰🇪');
  } else {
    if (badge) badge.textContent = 'English';
    if (menuText) menuText.textContent = 'Language (English / Kenyan)';
    showAppToast('Language Updated 🇬🇧', 'App language set to English (Default).', 'success', '🌐');
  }
}

function getCurrentAppLanguage() {
  return currentAppLanguage;
}

window.currentAppLanguage = currentAppLanguage;
window.getCurrentAppLanguage = getCurrentAppLanguage;
window.openLanguageModal = openLanguageModal;
window.closeLanguageModal = closeLanguageModal;
window.selectAppLanguage = selectAppLanguage;
window.updateLanguageModalUI = updateLanguageModalUI;
window.confirmAppLanguage = confirmAppLanguage;`;

const newJsLanguage = `/* ═════════════════════════════════════════════════════════════════════════════
   PATIENT APP LANGUAGE LOCALIZATION (ENGLISH & SWAHILI)
   ═════════════════════════════════════════════════════════════════════════════ */
let currentAppLanguage = 'en';

function openLanguageModal() {
  updateLanguageModalUI();
  const modal = document.getElementById('languageModal');
  if (modal) modal.style.display = 'flex';
}

function closeLanguageModal() {
  const modal = document.getElementById('languageModal');
  if (modal) modal.style.display = 'none';
}

function selectAppLanguage(langKey) {
  currentAppLanguage = langKey;
  window.currentAppLanguage = langKey;
  updateLanguageModalUI();
}

function updateLanguageModalUI() {
  const isEn = currentAppLanguage === 'en';
  const optEn = document.getElementById('langOptEnglish');
  const optSw = document.getElementById('langOptSwahili') || document.getElementById('langOptKenyan');
  const checkEn = document.getElementById('langCheckEnglish');
  const checkSw = document.getElementById('langCheckSwahili') || document.getElementById('langCheckKenyan');

  if (optEn) {
    optEn.style.borderColor = isEn ? 'var(--primary)' : 'var(--border)';
    optEn.style.background = isEn ? 'var(--primary-light)' : 'var(--surface)';
  }
  if (optSw) {
    optSw.style.borderColor = !isEn ? 'var(--primary)' : 'var(--border)';
    optSw.style.background = !isEn ? 'var(--primary-light)' : 'var(--surface)';
  }
  if (checkEn) checkEn.style.display = isEn ? 'block' : 'none';
  if (checkSw) checkSw.style.display = !isEn ? 'block' : 'none';
}

function confirmAppLanguage() {
  closeLanguageModal();
  window.currentAppLanguage = currentAppLanguage;
  const badge = document.getElementById('patientActiveLangBadge');
  const menuText = document.getElementById('patientLangMenuText');
  if (currentAppLanguage === 'sw' || currentAppLanguage === 'ke') {
    if (badge) badge.textContent = 'Swahili (Kiswahili)';
    if (menuText) menuText.textContent = 'Language (English / Swahili)';
    showAppToast('Lugha Imebadilishwa 🇰🇪', 'App language set to Swahili (Kiswahili). Karibu HelloDoctor!', 'success', '🇰🇪');
  } else {
    if (badge) badge.textContent = 'English';
    if (menuText) menuText.textContent = 'Language (English / Swahili)';
    showAppToast('Language Updated 🇬🇧', 'App language set to English (Default).', 'success', '🌐');
  }
}

function getCurrentAppLanguage() {
  return currentAppLanguage;
}

window.currentAppLanguage = currentAppLanguage;
window.getCurrentAppLanguage = getCurrentAppLanguage;
window.openLanguageModal = openLanguageModal;
window.closeLanguageModal = closeLanguageModal;
window.selectAppLanguage = selectAppLanguage;
window.updateLanguageModalUI = updateLanguageModalUI;
window.confirmAppLanguage = confirmAppLanguage;`;

if (html.includes(oldJsLanguage)) {
  html = html.replace(oldJsLanguage, newJsLanguage);
  console.log("✓ Updated JavaScript language controller to Swahili");
} else {
  console.error("✗ Could not find oldJsLanguage");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("Saved index.html successfully! New size:", html.length);
