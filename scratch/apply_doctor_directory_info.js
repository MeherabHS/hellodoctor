const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original file size:", html.length);

// 1. Update the header of adminDoctorHistoryModal
const oldModalHeader = `<div id="adhDocAvatar" style="width:44px; height:44px; border-radius:50%; background:#059669; color:#fff; display:grid; place-items:center; font-weight:800; font-size:15px;">SA</div>
          <div>
            <div style="display:flex; align-items:center; gap:8px;">
              <span id="adhDocName" style="font-size:16px; font-weight:800;">Dr. Sabrina Akter</span>
              <span class="badge badge-green" id="adhDocBmdc">BMDC #45821</span>
              <span class="badge badge-blue">Verified Specialist</span>
            </div>
            <div id="adhDocMeta" style="font-size:11.5px; color:#94A3B8; margin-top:2px;">Internal Medicine Consultant • Apollo Hospitals Dhaka • BMDC Registered Specialist</div>
          </div>`;

const newModalHeader = `<div id="adhDocAvatar" style="width:48px; height:48px; border-radius:50%; background:#059669; color:#fff; display:grid; place-items:center; font-weight:800; font-size:16px; flex-shrink:0;">SA</div>
          <div>
            <div style="display:flex; align-items:center; gap:8px; flex-wrap:wrap;">
              <span id="adhDocName" style="font-size:16.5px; font-weight:800; color:#FFFFFF;">Dr. Sabrina Akter</span>
              <span class="badge badge-green" id="adhDocBmdc">BMDC #45821</span>
              <span class="badge badge-blue">Verified Specialist</span>
              <span class="badge" style="background:rgba(255,255,255,0.12); color:#E2E8F0; font-size:10px;">Active Practitioner</span>
            </div>
            <div id="adhDocMeta" style="font-size:11.5px; color:#94A3B8; margin-top:2px;">Internal Medicine Consultant • Apollo Hospitals Dhaka • BMDC Registered Specialist</div>
            <div style="display:flex; align-items:center; gap:14px; margin-top:6px; font-size:11px; flex-wrap:wrap;">
              <span style="display:inline-flex; align-items:center; gap:4px; color:#94A3B8;">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#38BDF8" stroke-width="2.5"><path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/></svg>
                <span>Phone:</span>
                <b id="adhDocPhone" style="color:#FFFFFF; font-family:monospace; letter-spacing:0.2px;">+880 1713-445566</b>
              </span>
              <span style="color:#475569;">•</span>
              <span style="display:inline-flex; align-items:center; gap:4px; color:#94A3B8;">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#F59E0B" stroke-width="2.5"><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/><circle cx="12" cy="10" r="3"/></svg>
                <span>Residence:</span>
                <b id="adhDocResidence" style="color:#FFFFFF;">Banani, Dhaka (Road 11, Block D)</b>
              </span>
              <span style="color:#475569;">•</span>
              <span style="display:inline-flex; align-items:center; gap:4px; color:#94A3B8;">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="#10B981" stroke-width="2.5"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/><polyline points="22,6 12,13 2,6"/></svg>
                <span>Email:</span>
                <b id="adhDocEmail" style="color:#E2E8F0;">dr.sabrina.apollo@helodoc.com</b>
              </span>
            </div>
          </div>`;

if (html.includes(oldModalHeader)) {
  html = html.replace(oldModalHeader, newModalHeader);
  console.log("✓ Updated adminDoctorHistoryModal header with phone, residence & email elements");
}

// 2. Update adminDoctorHistoryStore with phone, residence and email
const oldAnikaStore = `anika: {
          name: 'Dr. Anika Rahman',
          bmdc: 'BMDC #A-74921',
          avatar: 'AR',
          bgColor: '#059669',
          meta: 'Pediatric Specialist • Dhaka Medical College Hospital • BMDC Registered Specialist',`;

const newAnikaStore = `anika: {
          name: 'Dr. Anika Rahman',
          bmdc: 'BMDC #A-74921',
          phone: '+880 1711-884920',
          residence: 'Dhanmondi, Dhaka (House 42, Road 7A)',
          email: 'dr.anika.dmch@helodoc.com',
          avatar: 'AR',
          bgColor: '#059669',
          meta: 'Pediatric Specialist • Dhaka Medical College Hospital • BMDC Registered Specialist',`;

if (html.includes(oldAnikaStore)) {
  html = html.replace(oldAnikaStore, newAnikaStore);
  console.log("✓ Added phone & residence to Dr. Anika store");
}

const oldSadikStore = `sadik: {
          name: 'Dr. Sadik Al-Amin',
          bmdc: 'BMDC #A-68192',
          avatar: 'SA',
          bgColor: '#2563EB',
          meta: 'General Medicine & Diabetology • BSMMU • BMDC Registered Specialist',`;

const newSadikStore = `sadik: {
          name: 'Dr. Sadik Al-Amin',
          bmdc: 'BMDC #A-68192',
          phone: '+880 1819-334455',
          residence: 'Gulshan-2, Dhaka (Avenue 3, Block C)',
          email: 'dr.sadik.bsmmu@helodoc.com',
          avatar: 'SA',
          bgColor: '#2563EB',
          meta: 'General Medicine & Diabetology • BSMMU • BMDC Registered Specialist',`;

if (html.includes(oldSadikStore)) {
  html = html.replace(oldSadikStore, newSadikStore);
  console.log("✓ Added phone & residence to Dr. Sadik store");
}

const oldFarhanaStore = `farhana: {
          name: 'Dr. Farhana Yesmin',
          bmdc: 'BMDC #A-53419',
          avatar: 'FY',
          bgColor: '#7C3AED',
          meta: 'Gynaecology & Obstetrics • BIRDEM Hospital • BMDC Registered Specialist',`;

const newFarhanaStore = `farhana: {
          name: 'Dr. Farhana Yesmin',
          bmdc: 'BMDC #A-53419',
          phone: '+880 1912-778899',
          residence: 'Uttara Sector 4, Dhaka (Road 11)',
          email: 'dr.farhana.birdem@helodoc.com',
          avatar: 'FY',
          bgColor: '#7C3AED',
          meta: 'Gynaecology & Obstetrics • BIRDEM Hospital • BMDC Registered Specialist',`;

if (html.includes(oldFarhanaStore)) {
  html = html.replace(oldFarhanaStore, newFarhanaStore);
  console.log("✓ Added phone & residence to Dr. Farhana store");
}

const oldSabrinaStore = `sabrina: {
          name: 'Dr. Sabrina Akter',
          bmdc: 'BMDC #45821',
          avatar: 'SA',
          bgColor: '#D97706',
          meta: 'Internal Medicine Consultant • Apollo Hospitals Dhaka • BMDC Registered Specialist',`;

const newSabrinaStore = `sabrina: {
          name: 'Dr. Sabrina Akter',
          bmdc: 'BMDC #45821',
          phone: '+880 1713-445566',
          residence: 'Banani, Dhaka (Road 11, Block D)',
          email: 'dr.sabrina.apollo@helodoc.com',
          avatar: 'SA',
          bgColor: '#D97706',
          meta: 'Internal Medicine Consultant • Apollo Hospitals Dhaka • BMDC Registered Specialist',`;

if (html.includes(oldSabrinaStore)) {
  html = html.replace(oldSabrinaStore, newSabrinaStore);
  console.log("✓ Added phone & residence to Dr. Sabrina store");
}

// 3. Update openAdminDoctorHistoryModal function to populate phone & residence
const oldOpenDocModal = `const mtEl = document.getElementById('adhDocMeta');
        const vEl = document.getElementById('adhKpiVisits');`;

const newOpenDocModal = `const mtEl = document.getElementById('adhDocMeta');
        const phEl = document.getElementById('adhDocPhone');
        const rsEl = document.getElementById('adhDocResidence');
        const emEl = document.getElementById('adhDocEmail');
        const vEl = document.getElementById('adhKpiVisits');`;

if (html.includes(oldOpenDocModal)) {
  html = html.replace(oldOpenDocModal, newOpenDocModal);
  console.log("✓ Updated openAdminDoctorHistoryModal element lookup");
}

const oldPopulateDoc = `if (mtEl) mtEl.textContent = doc.meta;
        if (vEl) vEl.textContent = doc.kpiVisits;`;

const newPopulateDoc = `if (mtEl) mtEl.textContent = doc.meta;
        if (phEl) phEl.textContent = doc.phone || '+880 1711-884920';
        if (rsEl) rsEl.textContent = doc.residence || 'Dhaka, Bangladesh';
        if (emEl) emEl.textContent = doc.email || 'doctor@helodoc.com';
        if (vEl) vEl.textContent = doc.kpiVisits;`;

if (html.includes(oldPopulateDoc)) {
  html = html.replace(oldPopulateDoc, newPopulateDoc);
  console.log("✓ Updated openAdminDoctorHistoryModal field population");
}

// 4. Also enrich the table rows in #adminDoctorsTableBody with phone & location
const oldAnikaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Anika Rahman</strong>
                          <div style="font-size:10.5px; color:#64748B;">Pediatric Specialist</div>`;

const newAnikaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Anika Rahman</strong>
                          <div style="font-size:10.5px; color:#64748B;">Pediatric Specialist • Dhanmondi, Dhaka</div>
                          <div style="font-size:10px; color:#2563EB; font-family:monospace; margin-top:1px;">📞 +880 1711-884920</div>`;

if (html.includes(oldAnikaRow)) {
  html = html.replace(oldAnikaRow, newAnikaRow);
  console.log("✓ Enriched Dr. Anika table row");
}

const oldSadikRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Sadik Al-Amin</strong>
                          <div style="font-size:10.5px; color:#64748B;">General Medicine &amp; Diabetology</div>`;

const newSadikRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Sadik Al-Amin</strong>
                          <div style="font-size:10.5px; color:#64748B;">General Medicine • Gulshan-2, Dhaka</div>
                          <div style="font-size:10px; color:#2563EB; font-family:monospace; margin-top:1px;">📞 +880 1819-334455</div>`;

if (html.includes(oldSadikRow)) {
  html = html.replace(oldSadikRow, newSadikRow);
  console.log("✓ Enriched Dr. Sadik table row");
}

const oldFarhanaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Farhana Yeasmin</strong>
                          <div style="font-size:10.5px; color:#64748B;">Gynaecology &amp; Obstetrics</div>`;

const newFarhanaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Farhana Yeasmin</strong>
                          <div style="font-size:10.5px; color:#64748B;">Gynaecology &amp; Obstetrics • Uttara, Dhaka</div>
                          <div style="font-size:10px; color:#2563EB; font-family:monospace; margin-top:1px;">📞 +880 1912-778899</div>`;

if (html.includes(oldFarhanaRow)) {
  html = html.replace(oldFarhanaRow, newFarhanaRow);
  console.log("✓ Enriched Dr. Farhana table row");
}

const oldSabrinaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Sabrina Akter</strong>
                          <div style="font-size:10.5px; color:#64748B;">Internal Medicine Consultant</div>`;

const newSabrinaRow = `<strong style="color:#0F172A; font-size:13px;">Dr. Sabrina Akter</strong>
                          <div style="font-size:10.5px; color:#64748B;">Internal Medicine • Banani, Dhaka</div>
                          <div style="font-size:10px; color:#2563EB; font-family:monospace; margin-top:1px;">📞 +880 1713-445566</div>`;

if (html.includes(oldSabrinaRow)) {
  html = html.replace(oldSabrinaRow, newSabrinaRow);
  console.log("✓ Enriched Dr. Sabrina table row");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully! New file size:", html.length);
