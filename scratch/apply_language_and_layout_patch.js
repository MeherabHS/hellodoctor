const fs = require('fs');

const targetFile = 'index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original index.html file size:", html.length);

// 1. Move Helpline from above Core Services to directly above Upcoming Consultation
const oldHomeChunk = `            <!-- Search Pill -->
            <div class="search-pill">
              <svg width="17" height="17" fill="none" stroke="currentColor" stroke-width="2"><circle cx="7.5" cy="7.5" r="5.5"/><path d="m15 15-3.5-3.5"/></svg>
              <input type="text" placeholder="Search specialist doctors, specialties…" onclick="switchScreen(3)">
              <svg width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><line x1="3" y1="6" x2="13" y2="6"/><line x1="5" y1="10" x2="11" y2="10"/></svg>
            </div>

            <!-- 24/7 Patient Medical Helpline Strip -->
            <div style="margin:2px 18px 12px; background:linear-gradient(135deg, #064E3B 0%, #047857 100%); border-radius:14px; padding:9px 13px; display:flex; justify-content:space-between; align-items:center; color:#fff; box-shadow:0 2px 8px rgba(5,150,105,0.18);">
              <div style="display:flex; align-items:center; gap:8px;">
                <div style="width:28px; height:28px; border-radius:50%; background:rgba(255,255,255,0.2); display:grid; place-items:center; font-size:13px;">📞</div>
                <div>
                  <div style="font-size:11px; font-weight:800; letter-spacing:0.2px;">24/7 Medical Helpline</div>
                  <div style="font-size:9.5px; opacity:0.9;">Emergency triage &amp; patient assistance</div>
                </div>
              </div>
              <a href="tel:16263" style="background:#fff; color:#064E3B; font-size:11px; font-weight:800; padding:4px 9px; border-radius:8px; text-decoration:none; display:inline-flex; align-items:center; gap:4px;" onclick="event.stopPropagation(); showAppToast('Calling Helpline 📞', 'Connecting to 24/7 Medical Hotline: 16263', 'info', '📞');">
                <span>Call 16263</span>
              </a>
            </div>

            <!-- Section Header -->
            <div class="section-hdr">
              <span class="section-title">Core Services</span>
              <span class="section-link" onclick="switchScreen(1)">View All</span>
            </div>`;

const newHomeChunk = `            <!-- Search Pill -->
            <div class="search-pill">
              <svg width="17" height="17" fill="none" stroke="currentColor" stroke-width="2"><circle cx="7.5" cy="7.5" r="5.5"/><path d="m15 15-3.5-3.5"/></svg>
              <input type="text" placeholder="Search specialist doctors, specialties…" onclick="switchScreen(3)">
              <svg width="16" height="16" fill="none" stroke="currentColor" stroke-width="2"><line x1="3" y1="6" x2="13" y2="6"/><line x1="5" y1="10" x2="11" y2="10"/></svg>
            </div>

            <!-- Section Header -->
            <div class="section-hdr">
              <span class="section-title">Core Services</span>
              <span class="section-link" onclick="switchScreen(1)">View All</span>
            </div>`;

if (html.includes(oldHomeChunk)) {
  html = html.replace(oldHomeChunk, newHomeChunk);
  console.log("✓ Removed Helpline strip from above Core Services");
} else {
  console.error("✗ Could not find oldHomeChunk");
}

// 2. Insert Helpline directly ABOVE Upcoming Consultation & Remove Medication Reminder card
const oldUpcomingChunk = `            <!-- Upcoming Requested Consultation -->
            <div class="section-hdr">
              <span class="section-title">Upcoming Appointment</span>
              <span class="section-link" onclick="switchScreen(4)">Manage</span>
            </div>
            <div class="consult-card" onclick="openConsultReminder()">
              <div class="consult-doc-avatar" style="color:var(--primary);">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="7" r="4"/>
                  <path d="M5.5 21v-2a4.5 4.5 0 0 1 4.5-4.5h4a4.5 4.5 0 0 1 4.5 4.5v2"/>
                  <path d="M9 12v1.5a1.5 1.5 0 0 0 1.5 1.5h3a1.5 1.5 0 0 0 1.5-1.5V12" stroke-width="1.5"/>
                </svg>
              </div>
              <div class="consult-details">
                <div class="consult-header-row">
                  <div class="consult-doc-name">Dr. Sabrina Akter</div>
                  <span class="badge badge-green">Confirmed</span>
                </div>
                <div class="consult-doc-spec">Internal Medicine Specialist</div>
                <div class="consult-time-row">
                  <svg fill="none" stroke="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                  <span>Today at 4:30 PM • Video Consultation</span>
                </div>
              </div>
            </div>

            <div class="consult-card" onclick="openMedicineReminder()" style="margin-bottom:8px;">
              <div class="consult-doc-avatar" style="background:var(--accent-light); color:#B45309;">
                <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
                  <rect x="3" y="10.5" width="18" height="8.5" rx="4.25" transform="rotate(-45 12 12)"/>
                  <line x1="8.5" y1="8.5" x2="15.5" y2="15.5"/>
                  <circle cx="6" cy="18" r="1" fill="currentColor"/>
                </svg>
              </div>
              <div class="consult-details">
                <div class="consult-header-row">
                  <div class="consult-doc-name">Medication Reminder</div>
                  <span class="badge badge-amber">Due in 30m</span>
                </div>
                <div class="consult-doc-spec" style="color:var(--text-muted)">Azithromycin 500mg (1 tablet after meal)</div>
                <div class="consult-time-row">
                  <svg fill="none" stroke="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                  <span>Scheduled 5:00 PM • Day 3 of 5</span>
                </div>
              </div>
            </div>`;

const newUpcomingChunk = `            <!-- 24/7 Patient Emergency Medical Helpline Strip (Placed directly above Upcoming Consultation) -->
            <div id="patientEmergencyHelplineStrip" style="margin:14px 18px 6px; background:linear-gradient(135deg, #064E3B 0%, #047857 100%); border-radius:14px; padding:10px 14px; display:flex; justify-content:space-between; align-items:center; color:#fff; box-shadow:0 3px 10px rgba(5,150,105,0.2);">
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
            </div>

            <!-- Upcoming Requested Consultation -->
            <div class="section-hdr" style="padding-top:6px;">
              <span class="section-title">Upcoming Consultation</span>
              <span class="section-link" onclick="switchScreen(4)">Manage</span>
            </div>
            <div class="consult-card" onclick="openConsultReminder()" style="margin-bottom:12px;">
              <div class="consult-doc-avatar" style="color:var(--primary);">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="12" cy="7" r="4"/>
                  <path d="M5.5 21v-2a4.5 4.5 0 0 1 4.5-4.5h4a4.5 4.5 0 0 1 4.5 4.5v2"/>
                  <path d="M9 12v1.5a1.5 1.5 0 0 0 1.5 1.5h3a1.5 1.5 0 0 0 1.5-1.5V12" stroke-width="1.5"/>
                </svg>
              </div>
              <div class="consult-details">
                <div class="consult-header-row">
                  <div class="consult-doc-name">Dr. Sabrina Akter</div>
                  <span class="badge badge-green">Confirmed</span>
                </div>
                <div class="consult-doc-spec">Internal Medicine Specialist</div>
                <div class="consult-time-row">
                  <svg fill="none" stroke="currentColor" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                  <span>Today at 4:30 PM • Video Consultation</span>
                </div>
              </div>
            </div>`;

if (html.includes(oldUpcomingChunk)) {
  html = html.replace(oldUpcomingChunk, newUpcomingChunk);
  console.log("✓ Placed Helpline above Upcoming Consultation and removed Medication Reminder card");
} else {
  console.error("✗ Could not find oldUpcomingChunk");
}

// 3. Update Language menu item in Account Settings to English & Kenyan
const oldLangMenu = `              <div class="menu-item">
                <div class="menu-icon" style="color:var(--text-sub); display:flex; align-items:center; justify-content:center;">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
                </div>
                <div class="menu-text">Language (English / বাংলা)</div>
                <div class="menu-arrow">›</div>
              </div>`;

const newLangMenu = `              <div class="menu-item" onclick="openLanguageModal()" style="cursor:pointer;" id="patientLangMenuItem">
                <div class="menu-icon" style="color:var(--text-sub); display:flex; align-items:center; justify-content:center;">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>
                </div>
                <div class="menu-text" id="patientLangMenuText">Language (English / Kenyan)</div>
                <span class="badge badge-green" id="patientActiveLangBadge" style="font-size:10px; margin-right:6px; font-weight:750;">English</span>
                <div class="menu-arrow">›</div>
              </div>`;

if (html.includes(oldLangMenu)) {
  html = html.replace(oldLangMenu, newLangMenu);
  console.log("✓ Updated Account Language menu item to English / Kenyan with click handler");
} else {
  console.error("✗ Could not find oldLangMenu");
}

// 4. Add Language Selector Modal to Bottom Sheets
const modalInsertionTarget = `<!-- 2. Bottom Sheet Modal: Medication Reminder -->`;

const languageModalHtml = `<!-- 0. Bottom Sheet Modal: Language Selector (English & Kenyan) -->
        <div class="modal-backdrop" id="languageModal" style="display:none;" onclick="if(event.target===this) closeLanguageModal();">
          <div class="bottom-sheet" style="max-height:80%;">
            <div class="sheet-pill-handle"></div>
            <div class="sheet-header-top" style="margin-bottom:8px;">
              <span class="sheet-pill-tag" style="background:var(--primary-light); color:var(--primary-dark); font-weight:800;">
                🌐 App Localization
              </span>
              <button class="toast-dismiss-btn" onclick="closeLanguageModal()" title="Close">✕</button>
            </div>
            <div class="sheet-title" style="font-size:17px; margin-bottom:3px;">Select App Language</div>
            <div class="sheet-desc" style="font-size:11.5px; margin-bottom:14px; line-height:1.45;">
              Choose your preferred language for consultations, medical records, and physician text guidance.
            </div>

            <!-- Language Options List -->
            <div style="display:flex; flex-direction:column; gap:10px; margin-bottom:16px;">
              <!-- Option 1: English (Default) -->
              <div class="lang-option-card active" id="langOptEnglish" onclick="selectAppLanguage('en')" style="display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-radius:14px; border:1.5px solid var(--primary); background:var(--primary-light); cursor:pointer; transition:all 0.15s ease;">
                <div style="display:flex; align-items:center; gap:12px;">
                  <div style="font-size:22px; line-height:1;">🇬🇧</div>
                  <div>
                    <div style="font-size:13px; font-weight:800; color:var(--text-main);">English</div>
                    <div style="font-size:10.5px; color:var(--text-sub);">International Standard (Default)</div>
                  </div>
                </div>
                <div class="lang-check" id="langCheckEnglish" style="color:var(--primary); font-weight:900; font-size:16px;">✓</div>
              </div>

              <!-- Option 2: Kenyan (Kiswahili / Kenyan English) -->
              <div class="lang-option-card" id="langOptKenyan" onclick="selectAppLanguage('ke')" style="display:flex; justify-content:space-between; align-items:center; padding:12px 14px; border-radius:14px; border:1.5px solid var(--border); background:var(--surface); cursor:pointer; transition:all 0.15s ease;">
                <div style="display:flex; align-items:center; gap:12px;">
                  <div style="font-size:22px; line-height:1;">🇰🇪</div>
                  <div>
                    <div style="font-size:13px; font-weight:800; color:var(--text-main);">Kenyan (Kiswahili / English)</div>
                    <div style="font-size:10.5px; color:var(--text-sub);">Lugha ya Kiswahili na Kiingereza cha Kenya</div>
                  </div>
                </div>
                <div class="lang-check" id="langCheckKenyan" style="color:var(--primary); font-weight:900; font-size:16px; display:none;">✓</div>
              </div>
            </div>

            <div class="sheet-actions">
              <button class="sheet-btn secondary" onclick="closeLanguageModal()" style="flex:1;">Cancel</button>
              <button class="sheet-btn primary" onclick="confirmAppLanguage()" style="flex:2; font-weight:800;">Apply Language Settings ✓</button>
            </div>
          </div>
        </div>

        <!-- 2. Bottom Sheet Modal: Medication Reminder -->`;

if (html.includes(modalInsertionTarget)) {
  html = html.replace(modalInsertionTarget, languageModalHtml);
  console.log("✓ Added #languageModal with English and Kenyan options");
} else {
  console.error("✗ Could not find modalInsertionTarget");
}

// 5. Add JavaScript language switcher logic in Script Block 1
const jsInsertionTarget = `let currentAppLanguage = 'en';`;

if (!html.includes(jsInsertionTarget)) {
  const jsCodeTarget = `/* ═════════════════════════════════════════════════════════════════════════════
   DOCTOR EARNINGS & 20% DEBARRED PLATFORM SETTLEMENT CONTROLLER`;

  const languageJs = `/* ═════════════════════════════════════════════════════════════════════════════
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

window.currentAppLanguage = currentAppLanguage;
window.openLanguageModal = openLanguageModal;
window.closeLanguageModal = closeLanguageModal;
window.selectAppLanguage = selectAppLanguage;
window.updateLanguageModalUI = updateLanguageModalUI;
window.confirmAppLanguage = confirmAppLanguage;

`;

  if (html.includes(jsCodeTarget)) {
    html = html.replace(jsCodeTarget, languageJs + jsCodeTarget);
    console.log("✓ Added Patient App Language controller functions to script");
  } else {
    console.error("✗ Could not find jsCodeTarget");
  }
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("Saved index.html successfully! New file size:", html.length);

// Also sync prototype/index.html if prototype dir exists
if (fs.existsSync('prototype')) {
  fs.writeFileSync('prototype/index.html', html, 'utf8');
  console.log("✓ Also synchronized prototype/index.html");
}
