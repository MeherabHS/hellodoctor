const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original file size:", html.length);

// 1. Add CSS for Doctor Earnings Breakdown & Debarment UI
const cssTarget = `.rx-med-dose { font-size: 10px; color: var(--text-sub); margin-top: 1px; }`;

const cssAddition = `.rx-med-dose { font-size: 10px; color: var(--text-sub); margin-top: 1px; }

/* ── Doctor Transparent Earnings & 20% Debarred Settlement UI ── */
.doc-earnings-breakdown-card {
  margin: 0 18px 16px;
  background: var(--surface);
  border: 1.5px solid var(--border);
  border-radius: 18px;
  padding: 16px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04);
}
.doc-earnings-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding-bottom: 10px;
  border-bottom: 1px dashed var(--border);
}
.doc-earnings-title {
  font-size: 13.5px;
  font-weight: 800;
  color: var(--text-main);
  display: flex;
  align-items: center;
  gap: 6px;
}
.doc-earnings-escrow-badge {
  font-size: 10.5px;
  font-weight: 700;
  background: #ECFDF5;
  color: #059669;
  border: 1px solid #A7F3D0;
  padding: 2px 8px;
  border-radius: 12px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.doc-earnings-step {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-radius: 10px;
  margin-bottom: 6px;
  background: var(--surface-subtle);
  border: 1px solid transparent;
}
.doc-earnings-step.gross {
  background: #EFF6FF;
  border-color: #BFDBFE;
}
.doc-earnings-step.withheld {
  background: #FEF2F2;
  border-color: #FECACA;
}
.doc-earnings-step.final-net {
  background: linear-gradient(135deg, #ECFDF5 0%, #D1FAE5 100%);
  border: 1.5px solid #10B981;
  padding: 10px 14px;
  margin-top: 8px;
  box-shadow: 0 2px 8px rgba(16, 185, 129, 0.15);
}
.doc-earnings-step-left {
  display: flex;
  align-items: center;
  gap: 10px;
}
.doc-earnings-step-icon {
  font-size: 17px;
  line-height: 1;
}
.doc-earnings-step-label {
  font-size: 12px;
  font-weight: 750;
  color: var(--text-main);
}
.doc-earnings-step-sub {
  font-size: 10px;
  color: var(--text-sub);
  margin-top: 1px;
}
.doc-earnings-step-val {
  font-size: 13.5px;
  font-weight: 800;
  text-align: right;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace;
}
.doc-earnings-step-val.gross {
  color: #1D4ED8;
}
.doc-earnings-step-val.withheld {
  color: #DC2626;
  display: flex;
  align-items: center;
  gap: 5px;
}
.doc-earnings-step-val.final-net {
  color: #065F46;
  font-size: 19px;
  font-weight: 850;
}
.doc-fee-pill {
  font-size: 9.5px;
  font-weight: 800;
  background: #FEE2E2;
  color: #B91C1C;
  padding: 1px 6px;
  border-radius: 6px;
  border: 1px solid #FECACA;
}
.doc-earnings-calc-note {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 10px;
  background: #F8FAFC;
  border: 1px solid #E2E8F0;
  color: #334155;
  font-size: 11px;
  line-height: 1.45;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.doc-earnings-calc-note svg {
  flex-shrink: 0;
  margin-top: 2px;
  color: #2563EB;
}
.doc-consult-fee-breakdown {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 4px;
  font-size: 10.5px;
  color: var(--text-sub);
  flex-wrap: wrap;
}
.doc-fee-formula-tag {
  background: #F1F5F9;
  border: 1px solid #CBD5E1;
  border-radius: 5px;
  padding: 1px 5px;
  font-family: monospace;
  font-size: 9.5px;
  color: #475569;
}`;

if (html.includes(cssTarget)) {
  html = html.replace(cssTarget, cssAddition);
  console.log("✓ Added Doctor Earnings CSS");
} else {
  console.error("✗ Could not find cssTarget");
}

// 2. Replace #doc-view-3 content
const oldDocView3 = `<div class="screen-panel" id="doc-view-3">
          <div class="dir-header">
            <div class="dir-title">Doctor Earnings & Wallet</div>
            <div class="dir-sub">Instant bKash/Bank settlement & earnings statement</div>
          </div>
          <div class="scroll-area" style="padding-bottom:90px;">
            <div class="vault-hero" style="background:linear-gradient(135deg, var(--surface) 0%, var(--surface-alt) 100%);">
              <div class="vh-title" style="font-size:12px;">Available Balance</div>
              <div class="vh-metric" style="color:#059669; font-size:28px;">৳ 28,450</div>
              <div class="vh-sub">Ready to transfer to bKash / City Bank</div>
              <div style="display:flex; gap:8px; justify-content:center; margin-top:12px;">
                <button class="sheet-btn primary" style="padding:8px 18px; font-size:12px;" onclick="showAppToast('Withdrawal Dispatched! 💸', '৳ 28,450 transferred to linked bKash number 01712-XXXXXX.', 'success', '💸')">Transfer to bKash 💸</button>
                <button class="sheet-btn secondary" style="padding:8px 14px; font-size:12px;" onclick="showAppToast('Statement Exported 📄', 'Monthly consultation earnings statement saved as PDF.', 'info', '📄')">Statement</button>
              </div>
            </div>
            <div class="section-hdr" style="padding:14px 18px 8px;">
              <span class="section-title">Recent Consultations & Payouts</span>
            </div>
            <div style="padding:0 18px;">
              <div class="patient-queue-card" style="margin:0 0 10px; padding:12px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px;">
                <div style="display:flex; justify-content:space-between; align-items:center;">
                  <div><strong>Rafiq Ahmed</strong><div style="font-size:11px; color:var(--text-sub);">Video Consultation • Today, 4:00 PM</div></div>
                  <div style="font-weight:800; color:#059669;">+ ৳ 630</div>
                </div>
              </div>
              <div class="patient-queue-card" style="margin:0 0 10px; padding:12px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px;">
                <div style="display:flex; justify-content:space-between; align-items:center;">
                  <div><strong>Nusrat Jahan</strong><div style="font-size:11px; color:var(--text-sub);">Follow-up Triage • Yesterday</div></div>
                  <div style="font-weight:800; color:#059669;">+ ৳ 450</div>
                </div>
              </div>
              <div class="patient-queue-card" style="margin:0 0 10px; padding:12px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px;">
                <div style="display:flex; justify-content:space-between; align-items:center;">
                  <div><strong>Tanvir Hossain</strong><div style="font-size:11px; color:var(--text-sub);">Instant Chat Consultation • 21 Sep</div></div>
                  <div style="font-weight:800; color:#059669;">+ ৳ 350</div>
                </div>
              </div>
            </div>
          </div>
        </div>`;

const newDocView3 = `<div class="screen-panel" id="doc-view-3">
          <div class="dir-header">
            <div class="dir-title">Doctor Earnings &amp; Wallet</div>
            <div class="dir-sub">Transparent 20% platform charge reconciliation &amp; instant settlements</div>
          </div>
          <div class="scroll-area" style="padding-bottom:90px;">
            
            <!-- Transparent Earnings Breakdown Card (Gross -> 20% Withheld -> Final Earning) -->
            <div class="doc-earnings-breakdown-card" id="docEarningsHeroCard">
              <div class="doc-earnings-header">
                <div class="doc-earnings-title">
                  <span>Settlement Breakdown</span>
                  <span class="doc-fee-pill" style="font-size:10px;">Cycle: Sep 2026</span>
                </div>
                <span class="doc-earnings-escrow-badge">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                  <span>Escrow Cleared</span>
                </span>
              </div>

              <!-- Step 1: Total Earnings (Gross) -->
              <div class="doc-earnings-step gross" id="docEarningsGrossRow">
                <div class="doc-earnings-step-left">
                  <span class="doc-earnings-step-icon">💰</span>
                  <div>
                    <div class="doc-earnings-step-label">1. Total Earnings (Gross)</div>
                    <div class="doc-earnings-step-sub">Patient fees before platform commission (42 consults)</div>
                  </div>
                </div>
                <div class="doc-earnings-step-val gross" id="docGrossEarningsVal">৳ 35,562.50</div>
              </div>

              <!-- Step 2: Debarred Platform Charge (20% Withheld) -->
              <div class="doc-earnings-step withheld" id="docEarningsWithheldRow">
                <div class="doc-earnings-step-left">
                  <span class="doc-earnings-step-icon">📉</span>
                  <div>
                    <div class="doc-earnings-step-label">2. Platform Charge (20% Withheld)</div>
                    <div class="doc-earnings-step-sub">Debarred platform commission &amp; operational fee</div>
                  </div>
                </div>
                <div class="doc-earnings-step-val withheld" id="docWithheldFeeVal">
                  <span>- ৳ 7,112.50</span>
                  <span class="doc-fee-pill">-20%</span>
                </div>
              </div>

              <!-- Step 3: Final Earning (Net Disbursable) -->
              <div class="doc-earnings-step final-net" id="docEarningsFinalRow">
                <div class="doc-earnings-step-left">
                  <span class="doc-earnings-step-icon">💵</span>
                  <div>
                    <div class="doc-earnings-step-label" style="font-size:13px; color:#064E3B;">3. Final Earning (Net Take-Home)</div>
                    <div class="doc-earnings-step-sub" style="color:#047857; font-weight:600;">Cleared balance available for instant withdrawal</div>
                  </div>
                </div>
                <div class="doc-earnings-step-val final-net" id="docFinalEarningsVal">৳ 28,450.00</div>
              </div>

              <!-- Small Explanatory Calculation Note (Strict Requirement) -->
              <div class="doc-earnings-calc-note" id="docEarningsCalcNote">
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
                <div>
                  <strong>Fee Calculation Basis:</strong> Total calculation is based including the platform charge 20%. The 20% platform charge covers payment gateway interchange (bKash/Nagad/Cards), automated escrow dispute insurance, WebRTC HD teleconsultation infrastructure, and 24/7 technical support.
                </div>
              </div>

              <!-- Quick Payout Actions -->
              <div style="display:flex; gap:8px; justify-content:center; margin-top:14px;">
                <button class="sheet-btn primary" style="padding:9px 18px; font-size:12px; font-weight:750; flex:1;" onclick="showAppToast('Withdrawal Dispatched! 💸', '৳ 28,450.00 (Net Final Earning after 20% platform fee debarment) transferred to linked bKash merchant 01713-445566.', 'success', '💸')">Transfer to bKash 💸</button>
                <button class="sheet-btn secondary" style="padding:9px 14px; font-size:12px; font-weight:700;" onclick="openDoctorStatementModal()">Statement 📄</button>
              </div>
            </div>

            <!-- Recent Consultations Header -->
            <div class="section-hdr" style="padding:8px 18px 8px; display:flex; justify-content:space-between; align-items:center;">
              <span class="section-title">Consultation Ledger &amp; 20% Debarment</span>
              <span style="font-size:11px; color:var(--text-sub);">Formula: Total - 20% = Final</span>
            </div>

            <div style="padding:0 18px; display:flex; flex-direction:column; gap:10px;" id="docConsultationsLedgerContainer">
              <!-- Item 1: Rafiq Ahmed -->
              <div class="patient-queue-card" style="margin:0; padding:12px 14px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px; cursor:pointer;" onclick="openConsultationFeeDetailModal('Rafiq Ahmed', 'Video Consultation (10 min)', 800, 160, 640, 'BK-MER-7718290')">
                <div style="display:flex; justify-content:space-between; align-items:flex-start;">
                  <div>
                    <div style="display:flex; align-items:center; gap:6px;">
                      <strong style="font-size:13px; color:var(--text-main);">Rafiq Ahmed</strong>
                      <span class="badge badge-green" style="font-size:9px; padding:1px 6px;">Settled ✓</span>
                    </div>
                    <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Video Consultation • Today, 4:00 PM</div>
                    <div class="doc-consult-fee-breakdown">
                      <span>Total: <b style="color:var(--text-main);">৳ 800</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span style="color:#DC2626;">20% Fee: <b>- ৳ 160</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span class="doc-fee-formula-tag">Debarred 20%</span>
                    </div>
                  </div>
                  <div style="text-align:right;">
                    <div style="font-weight:800; color:#059669; font-size:14px;">+ ৳ 640</div>
                    <div style="font-size:9.5px; color:#64748B; margin-top:1px;">Final Earning</div>
                  </div>
                </div>
              </div>

              <!-- Item 2: Nusrat Jahan -->
              <div class="patient-queue-card" style="margin:0; padding:12px 14px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px; cursor:pointer;" onclick="openConsultationFeeDetailModal('Nusrat Jahan', 'Follow-up Triage & Thyroid Review', 600, 120, 480, 'NG-TXN-102948')">
                <div style="display:flex; justify-content:space-between; align-items:flex-start;">
                  <div>
                    <div style="display:flex; align-items:center; gap:6px;">
                      <strong style="font-size:13px; color:var(--text-main);">Nusrat Jahan</strong>
                      <span class="badge badge-green" style="font-size:9px; padding:1px 6px;">Settled ✓</span>
                    </div>
                    <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Follow-up Triage • Yesterday, 3:45 PM</div>
                    <div class="doc-consult-fee-breakdown">
                      <span>Total: <b style="color:var(--text-main);">৳ 600</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span style="color:#DC2626;">20% Fee: <b>- ৳ 120</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span class="doc-fee-formula-tag">Debarred 20%</span>
                    </div>
                  </div>
                  <div style="text-align:right;">
                    <div style="font-weight:800; color:#059669; font-size:14px;">+ ৳ 480</div>
                    <div style="font-size:9.5px; color:#64748B; margin-top:1px;">Final Earning</div>
                  </div>
                </div>
              </div>

              <!-- Item 3: Tanvir Hossain -->
              <div class="patient-queue-card" style="margin:0; padding:12px 14px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px; cursor:pointer;" onclick="openConsultationFeeDetailModal('Tanvir Hossain', 'Instant Chat Consultation', 500, 100, 400, 'BK-MER-55219')">
                <div style="display:flex; justify-content:space-between; align-items:flex-start;">
                  <div>
                    <div style="display:flex; align-items:center; gap:6px;">
                      <strong style="font-size:13px; color:var(--text-main);">Tanvir Hossain</strong>
                      <span class="badge badge-green" style="font-size:9px; padding:1px 6px;">Settled ✓</span>
                    </div>
                    <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Instant Chat Consultation • 21 Sep</div>
                    <div class="doc-consult-fee-breakdown">
                      <span>Total: <b style="color:var(--text-main);">৳ 500</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span style="color:#DC2626;">20% Fee: <b>- ৳ 100</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span class="doc-fee-formula-tag">Debarred 20%</span>
                    </div>
                  </div>
                  <div style="text-align:right;">
                    <div style="font-weight:800; color:#059669; font-size:14px;">+ ৳ 400</div>
                    <div style="font-size:9.5px; color:#64748B; margin-top:1px;">Final Earning</div>
                  </div>
                </div>
              </div>

              <!-- Item 4: Kamal Uddin -->
              <div class="patient-queue-card" style="margin:0; padding:12px 14px; background:var(--surface); border:1.5px solid var(--border); border-radius:14px; cursor:pointer;" onclick="openConsultationFeeDetailModal('Kamal Uddin', 'Hypertension & Diabetes Review', 1000, 200, 800, 'SSL-CARD-773194')">
                <div style="display:flex; justify-content:space-between; align-items:flex-start;">
                  <div>
                    <div style="display:flex; align-items:center; gap:6px;">
                      <strong style="font-size:13px; color:var(--text-main);">Kamal Uddin</strong>
                      <span class="badge badge-green" style="font-size:9px; padding:1px 6px;">Settled ✓</span>
                    </div>
                    <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Hypertension &amp; Diabetes Review • 19 Sep</div>
                    <div class="doc-consult-fee-breakdown">
                      <span>Total: <b style="color:var(--text-main);">৳ 1,000</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span style="color:#DC2626;">20% Fee: <b>- ৳ 200</b></span>
                      <span style="color:#94A3B8;">•</span>
                      <span class="doc-fee-formula-tag">Debarred 20%</span>
                    </div>
                  </div>
                  <div style="text-align:right;">
                    <div style="font-weight:800; color:#059669; font-size:14px;">+ ৳ 800</div>
                    <div style="font-size:9.5px; color:#64748B; margin-top:1px;">Final Earning</div>
                  </div>
                </div>
              </div>
            </div>

          </div>
        </div>`;

if (html.includes(oldDocView3)) {
  html = html.replace(oldDocView3, newDocView3);
  console.log("✓ Replaced #doc-view-3 with 3-tier debarred earnings UI");
} else {
  console.error("✗ Could not find oldDocView3");
}

// 3. Add doctorStatementModal and docConsultationFeeModal
const modalTarget = `      </div><!-- /.screen-shell #appShell -->`;

const modalAddition = `      <!-- ═════════════════════════════════════════════════════════════
           DOCTOR EARNINGS & 20% DEBARRED PLATFORM SETTLEMENT STATEMENT MODAL
           ═════════════════════════════════════════════════════════════ -->
      <div class="modal-backdrop" id="doctorStatementModal" style="display:none;" onclick="if(event.target===this) closeDoctorStatementModal();">
        <div class="bottom-sheet" style="max-height:92%; display:flex; flex-direction:column;">
          <div class="sheet-pill-handle"></div>

          <div class="sheet-header-top" style="margin-bottom:6px;">
            <div style="display:flex; align-items:center; gap:6px;">
              <span class="sheet-pill-tag" style="background:#ECFDF5; color:#059669; font-weight:800;">
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
                Verified Settlement Statement
              </span>
              <span style="font-size:11px; font-weight:700; color:var(--text-sub);">Cycle: September 2026</span>
            </div>
            <button class="toast-dismiss-btn" onclick="closeDoctorStatementModal()" title="Close">✕</button>
          </div>

          <div class="sheet-title" style="font-size:16.5px; margin-bottom:2px;" id="dsmDocTitle">Physician Earnings &amp; Platform Debarment Statement</div>
          <div class="sheet-desc" style="font-size:11.5px; margin-bottom:12px; line-height:1.4;">
            Official reconciliation statement for <strong>Dr. Sabrina Akter (BMDC #45821)</strong>. Itemized patient fees and 20% platform charge debarment.
          </div>

          <!-- Scrollable Statement Body -->
          <div style="flex:1; overflow-y:auto; padding-right:2px; margin-bottom:12px;">
            <!-- 3-Tier Summary Card -->
            <div style="background:var(--surface-subtle); border:1.5px solid var(--border); border-radius:14px; padding:12px 14px; margin-bottom:14px;">
              <div style="font-size:11px; font-weight:800; color:var(--text-sub); text-transform:uppercase; letter-spacing:0.3px; margin-bottom:8px;">Settlement Summary Formula</div>
              
              <!-- 1. Total Earnings -->
              <div style="display:flex; justify-content:space-between; align-items:center; padding:6px 0; border-bottom:1px dashed var(--border);">
                <div>
                  <span style="font-size:12px; font-weight:750; color:#1E40AF;">1. Total Earnings (Gross)</span>
                  <div style="font-size:10px; color:var(--text-sub);">All consultations &amp; triage services</div>
                </div>
                <strong style="font-size:13.5px; color:#1E40AF;" id="dsmGrossVal">৳ 35,562.50</strong>
              </div>

              <!-- 2. Debarred Platform Charge -->
              <div style="display:flex; justify-content:space-between; align-items:center; padding:6px 0; border-bottom:1px dashed var(--border);">
                <div>
                  <span style="font-size:12px; font-weight:750; color:#DC2626;">2. Less: 20% Platform Charge (Withheld)</span>
                  <div style="font-size:10px; color:var(--text-sub);">Payment gateway, escrow &amp; telehealth infrastructure</div>
                </div>
                <strong style="font-size:13.5px; color:#DC2626;" id="dsmWithheldVal">- ৳ 7,112.50</strong>
              </div>

              <!-- 3. Final Earning -->
              <div style="display:flex; justify-content:space-between; align-items:center; padding:8px 0 2px;">
                <div>
                  <span style="font-size:13px; font-weight:850; color:#065F46;">3. Final Earning (Net Disbursable)</span>
                  <div style="font-size:10px; color:#059669; font-weight:600;">Net credited to physician wallet</div>
                </div>
                <strong style="font-size:17px; color:#059669; font-weight:900;" id="dsmFinalVal">৳ 28,450.00</strong>
              </div>
            </div>

            <!-- Explanatory text banner (Strict Requirement) -->
            <div class="doc-earnings-calc-note" style="margin-bottom:14px;">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
              <div>
                <strong>Accounting Basis:</strong> Total calculation is based including the platform charge 20%. HeloDoc applies a standardized 20% operational charge on all patient consultations to maintain PCI-DSS certified payment escrow, WebRTC low-latency streaming servers, automated BMDC license validation, and 24/7 technical hotline dispatch.
              </div>
            </div>

            <!-- Itemized Table Header -->
            <div style="font-size:11px; font-weight:800; color:var(--text-sub); text-transform:uppercase; letter-spacing:0.3px; margin-bottom:8px;">
              Itemized Consultations (Formula: Gross - 20% = Net)
            </div>

            <!-- Itemized Table -->
            <div style="border:1px solid var(--border); border-radius:12px; overflow:hidden; background:var(--surface);">
              <table style="width:100%; border-collapse:collapse; font-size:11px; text-align:left;">
                <thead style="background:var(--surface-subtle); border-bottom:1px solid var(--border); color:var(--text-sub); font-size:10px; text-transform:uppercase;">
                  <tr>
                    <th style="padding:8px 10px;">Consultation</th>
                    <th style="padding:8px 10px; text-align:right;">Total (Gross)</th>
                    <th style="padding:8px 10px; text-align:right;">Debarred (20%)</th>
                    <th style="padding:8px 10px; text-align:right;">Final (Net)</th>
                  </tr>
                </thead>
                <tbody id="dsmItemizedTbody">
                  <tr style="border-bottom:1px solid var(--border); cursor:pointer;" onclick="openConsultationFeeDetailModal('Rafiq Ahmed', 'Video Consultation (10 min)', 800, 160, 640, 'BK-MER-7718290')">
                    <td style="padding:8px 10px;">
                      <strong style="color:var(--text-main); font-size:11.5px;">Rafiq Ahmed</strong>
                      <div style="font-size:9.5px; color:var(--text-sub);">Video Consultation (10 min) • Today, 4:00 PM</div>
                    </td>
                    <td style="padding:8px 10px; text-align:right; font-weight:700; color:#1E40AF;">৳ 800</td>
                    <td style="padding:8px 10px; text-align:right; color:#DC2626; font-weight:700;">- ৳ 160</td>
                    <td style="padding:8px 10px; text-align:right; font-weight:800; color:#059669;">+ ৳ 640</td>
                  </tr>
                  <tr style="border-bottom:1px solid var(--border); cursor:pointer;" onclick="openConsultationFeeDetailModal('Nusrat Jahan', 'Follow-up Triage &amp; Thyroid Review', 600, 120, 480, 'NG-TXN-102948')">
                    <td style="padding:8px 10px;">
                      <strong style="color:var(--text-main); font-size:11.5px;">Nusrat Jahan</strong>
                      <div style="font-size:9.5px; color:var(--text-sub);">Follow-up Triage • Yesterday, 3:45 PM</div>
                    </td>
                    <td style="padding:8px 10px; text-align:right; font-weight:700; color:#1E40AF;">৳ 600</td>
                    <td style="padding:8px 10px; text-align:right; color:#DC2626; font-weight:700;">- ৳ 120</td>
                    <td style="padding:8px 10px; text-align:right; font-weight:800; color:#059669;">+ ৳ 480</td>
                  </tr>
                  <tr style="border-bottom:1px solid var(--border); cursor:pointer;" onclick="openConsultationFeeDetailModal('Tanvir Hossain', 'Instant Chat Consultation', 500, 100, 400, 'BK-MER-55219')">
                    <td style="padding:8px 10px;">
                      <strong style="color:var(--text-main); font-size:11.5px;">Tanvir Hossain</strong>
                      <div style="font-size:9.5px; color:var(--text-sub);">Instant Chat Consultation • 21 Sep</div>
                    </td>
                    <td style="padding:8px 10px; text-align:right; font-weight:700; color:#1E40AF;">৳ 500</td>
                    <td style="padding:8px 10px; text-align:right; color:#DC2626; font-weight:700;">- ৳ 100</td>
                    <td style="padding:8px 10px; text-align:right; font-weight:800; color:#059669;">+ ৳ 400</td>
                  </tr>
                  <tr style="border-bottom:1px solid var(--border); cursor:pointer;" onclick="openConsultationFeeDetailModal('Kamal Uddin', 'Hypertension &amp; Diabetes Review', 1000, 200, 800, 'SSL-CARD-773194')">
                    <td style="padding:8px 10px;">
                      <strong style="color:var(--text-main); font-size:11.5px;">Kamal Uddin</strong>
                      <div style="font-size:9.5px; color:var(--text-sub);">Hypertension &amp; Diabetes Review • 19 Sep</div>
                    </td>
                    <td style="padding:8px 10px; text-align:right; font-weight:700; color:#1E40AF;">৳ 1,000</td>
                    <td style="padding:8px 10px; text-align:right; color:#DC2626; font-weight:700;">- ৳ 200</td>
                    <td style="padding:8px 10px; text-align:right; font-weight:800; color:#059669;">+ ৳ 800</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>

          <!-- Bottom Actions -->
          <div style="display:flex; gap:8px;">
            <button class="sheet-btn secondary" style="flex:1; padding:10px;" onclick="closeDoctorStatementModal()">Close</button>
            <button class="sheet-btn primary" style="flex:2; padding:10px; font-weight:800;" onclick="showAppToast('Official Statement Downloaded 📄', 'PDF statement with 20% platform charge breakdown saved to device.', 'success', '📄')">Download Statement (PDF) 📄</button>
          </div>
        </div>
      </div><!-- /#doctorStatementModal -->

      <!-- ═════════════════════════════════════════════════════════════
           SINGLE CONSULTATION FEE CALCULATION MODAL
           ═════════════════════════════════════════════════════════════ -->
      <div class="modal-backdrop" id="docConsultationFeeModal" style="display:none;" onclick="if(event.target===this) closeConsultationFeeModal();">
        <div class="bottom-sheet" style="max-height:85%;">
          <div class="sheet-pill-handle"></div>

          <div class="sheet-header-top" style="margin-bottom:6px;">
            <div style="display:flex; align-items:center; gap:6px;">
              <span class="sheet-pill-tag" style="background:#EFF6FF; color:#1D4ED8; font-weight:800;">
                🧾 Consultation Fee Audit
              </span>
              <span style="font-size:11px; font-weight:700; color:var(--text-sub);" id="dcfmTrxId">REF: BK-MER-7718290</span>
            </div>
            <button class="toast-dismiss-btn" onclick="closeConsultationFeeModal()" title="Close">✕</button>
          </div>

          <div class="sheet-title" style="font-size:16px; margin-bottom:2px;" id="dcfmPatientTitle">Fee Breakdown: Rafiq Ahmed</div>
          <div class="sheet-desc" style="font-size:11px; margin-bottom:12px;" id="dcfmSessionSub">Video Consultation (10 min) • Today, 4:00 PM</div>

          <!-- Step-by-Step Debarred Breakdown -->
          <div style="background:var(--surface-subtle); border:1.5px solid var(--border); border-radius:14px; padding:14px; margin-bottom:12px;">
            <div class="doc-earnings-step gross" style="margin-bottom:6px;">
              <div class="doc-earnings-step-left">
                <span class="doc-earnings-step-icon">💰</span>
                <div>
                  <div class="doc-earnings-step-label">1. Total Consultation Fee</div>
                  <div class="doc-earnings-step-sub">Patient paid via MFS gateway</div>
                </div>
              </div>
              <div class="doc-earnings-step-val gross" id="dcfmGross">৳ 800</div>
            </div>

            <div class="doc-earnings-step withheld" style="margin-bottom:6px;">
              <div class="doc-earnings-step-left">
                <span class="doc-earnings-step-icon">📉</span>
                <div>
                  <div class="doc-earnings-step-label">2. Debarred Platform Charge (20%)</div>
                  <div class="doc-earnings-step-sub">HeloDoc commission &amp; processing fee</div>
                </div>
              </div>
              <div class="doc-earnings-step-val withheld" id="dcfmWithheld">
                <span>- ৳ 160</span>
                <span class="doc-fee-pill">-20%</span>
              </div>
            </div>

            <div class="doc-earnings-step final-net">
              <div class="doc-earnings-step-left">
                <span class="doc-earnings-step-icon">💵</span>
                <div>
                  <div class="doc-earnings-step-label" style="color:#064E3B;">3. Final Net Earning</div>
                  <div class="doc-earnings-step-sub" style="color:#047857;">Credited directly to doctor wallet</div>
                </div>
              </div>
              <div class="doc-earnings-step-val final-net" id="dcfmNet">৳ 640</div>
            </div>
          </div>

          <div class="doc-earnings-calc-note" style="margin-bottom:14px;">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
            <div>
              <strong>Calculation Note:</strong> Total calculation is based including the platform charge 20%. The 20% platform charge is automatically debarred before wallet credit.
            </div>
          </div>

          <div style="display:flex; gap:8px;">
            <button class="sheet-btn primary" style="width:100%; padding:10px;" onclick="closeConsultationFeeModal()">Dismiss Audit Receipt ✓</button>
          </div>
        </div>
      </div><!-- /#docConsultationFeeModal -->
      </div><!-- /.screen-shell #appShell -->`;

if (html.includes(modalTarget)) {
  html = html.replace(modalTarget, modalAddition);
  console.log("✓ Added doctorStatementModal and docConsultationFeeModal before closing appShell");
} else {
  console.error("✗ Could not find modalTarget");
}

// 4. Update Doctor Web Shell Revenue Card
const oldWebRevenue = `<div class="dw-kpi-card">
              <div class="dw-kpi-top">
                <span class="dw-kpi-lbl">Daily Revenue (Est.)</span>
                <span class="dw-kpi-icon">💳</span>
              </div>
              <div class="dw-kpi-val">৳ 8,400</div>
              <div class="dw-kpi-sub">bKash & Nagad instant settlement</div>
            </div>`;

const newWebRevenue = `<div class="dw-kpi-card" onclick="openDoctorStatementModal()" style="cursor:pointer;" title="Click to view itemized 20% platform fee reconciliation">
              <div class="dw-kpi-top">
                <span class="dw-kpi-lbl">Doctor Earnings &amp; Settlements</span>
                <span class="dw-kpi-icon">💳</span>
              </div>
              <div style="display:flex; align-items:baseline; justify-content:space-between; margin-top:4px;">
                <span style="font-size:11px; color:#64748B;">Total Gross: <b>৳ 10,500</b></span>
                <span style="font-size:11px; color:#DC2626;">20% Fee: <b>- ৳ 2,100</b></span>
              </div>
              <div class="dw-kpi-val" style="color:#059669; margin-top:4px;">৳ 8,400 <span style="font-size:11px; font-weight:600; color:#059669;">Net Final</span></div>
              <div class="dw-kpi-sub" style="font-size:10px; line-height:1.35; margin-top:4px; color:#475569;">Total calculation is based including the platform charge 20% (Debarred for gateway &amp; infrastructure).</div>
            </div>`;

if (html.includes(oldWebRevenue)) {
  html = html.replace(oldWebRevenue, newWebRevenue);
  console.log("✓ Updated Doctor Web Shell revenue card with 20% debarment breakdown");
} else {
  console.error("✗ Could not find oldWebRevenue");
}

// 5. Add JavaScript Controllers in Script Block 1
const jsTarget = `function switchDoctorTab(tabIdx) {`;

const jsAddition = `/* ═════════════════════════════════════════════════════════════════════════════
   DOCTOR EARNINGS & 20% DEBARRED PLATFORM SETTLEMENT CONTROLLER
   ═════════════════════════════════════════════════════════════════════════════ */
const doctorEarningsStore = {
  doctorName: 'Dr. Sabrina Akter',
  bmdc: 'BMDC #45821',
  period: 'September 2026',
  platformChargePercent: 20,
  totalGross: 35562.50,
  withheldFee: 7112.50,
  finalNet: 28450.00,
  consultations: [
    {
      id: 'CONS-9340',
      patient: 'Rafiq Ahmed',
      service: 'Video Consultation (10 min)',
      time: 'Today, 4:00 PM',
      gross: 800,
      withheld: 160,
      net: 640,
      gatewayRef: 'BK-MER-7718290',
      status: 'Settled ✓'
    },
    {
      id: 'CONS-9328',
      patient: 'Nusrat Jahan',
      service: 'Follow-up Triage & Thyroid Review',
      time: 'Yesterday, 3:45 PM',
      gross: 600,
      withheld: 120,
      net: 480,
      gatewayRef: 'NG-TXN-102948',
      status: 'Settled ✓'
    },
    {
      id: 'CONS-9315',
      patient: 'Tanvir Hossain',
      service: 'Instant Chat Consultation',
      time: '21 Sep, 11:30 AM',
      gross: 500,
      withheld: 100,
      net: 400,
      gatewayRef: 'BK-MER-55219',
      status: 'Settled ✓'
    },
    {
      id: 'CONS-9302',
      patient: 'Kamal Uddin',
      service: 'Hypertension & Diabetes Review',
      time: '19 Sep, 02:15 PM',
      gross: 1000,
      withheld: 200,
      net: 800,
      gatewayRef: 'SSL-CARD-773194',
      status: 'Settled ✓'
    }
  ]
};

function renderDoctorEarnings() {
  const grossEl = document.getElementById('docGrossEarningsVal');
  const withheldEl = document.getElementById('docWithheldFeeVal');
  const finalEl = document.getElementById('docFinalEarningsVal');
  const dsmGrossEl = document.getElementById('dsmGrossVal');
  const dsmWithheldEl = document.getElementById('dsmWithheldVal');
  const dsmFinalEl = document.getElementById('dsmFinalVal');
  const itemizedTbody = document.getElementById('dsmItemizedTbody');

  const s = doctorEarningsStore;
  if (grossEl) grossEl.textContent = '৳ ' + s.totalGross.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (withheldEl) withheldEl.innerHTML = '<span>- ৳ ' + s.withheldFee.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 }) + '</span> <span class="doc-fee-pill">-' + s.platformChargePercent + '%</span>';
  if (finalEl) finalEl.textContent = '৳ ' + s.finalNet.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  if (dsmGrossEl) dsmGrossEl.textContent = '৳ ' + s.totalGross.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (dsmWithheldEl) dsmWithheldEl.textContent = '- ৳ ' + s.withheldFee.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (dsmFinalEl) dsmFinalEl.textContent = '৳ ' + s.finalNet.toLocaleString('en-IN', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  if (itemizedTbody && s.consultations) {
    itemizedTbody.innerHTML = s.consultations.map(function(c) {
      return '<tr style="border-bottom:1px solid var(--border); cursor:pointer;" onclick="openConsultationFeeDetailModal(\\'' + c.patient + '\\', \\'' + c.service + '\\', ' + c.gross + ', ' + c.withheld + ', ' + c.net + ', \\'' + c.gatewayRef + '\\')">' +
        '<td style="padding:8px 10px;">' +
          '<strong style="color:var(--text-main); font-size:11.5px;">' + c.patient + '</strong>' +
          '<div style="font-size:9.5px; color:var(--text-sub);">' + c.service + ' • ' + c.time + '</div>' +
        '</td>' +
        '<td style="padding:8px 10px; text-align:right; font-weight:700; color:#1E40AF;">৳ ' + c.gross + '</td>' +
        '<td style="padding:8px 10px; text-align:right; color:#DC2626; font-weight:700;">- ৳ ' + c.withheld + '</td>' +
        '<td style="padding:8px 10px; text-align:right; font-weight:800; color:#059669;">+ ৳ ' + c.net + '</td>' +
      '</tr>';
    }).join('');
  }
}

function openDoctorStatementModal() {
  renderDoctorEarnings();
  const modal = document.getElementById('doctorStatementModal');
  if (modal) {
    modal.style.display = 'flex';
  }
}

function closeDoctorStatementModal() {
  const modal = document.getElementById('doctorStatementModal');
  if (modal) {
    modal.style.display = 'none';
  }
}

function openConsultationFeeDetailModal(patient, service, gross, withheld, net, trxId) {
  const modal = document.getElementById('docConsultationFeeModal');
  if (!modal) return;
  const tEl = document.getElementById('dcfmPatientTitle');
  const sEl = document.getElementById('dcfmSessionSub');
  const rEl = document.getElementById('dcfmTrxId');
  const gEl = document.getElementById('dcfmGross');
  const wEl = document.getElementById('dcfmWithheld');
  const nEl = document.getElementById('dcfmNet');

  if (tEl) tEl.textContent = 'Fee Breakdown: ' + patient;
  if (sEl) sEl.textContent = service;
  if (rEl) rEl.textContent = 'REF: ' + (trxId || 'BK-MER-AUTO');
  if (gEl) gEl.textContent = '৳ ' + gross;
  if (wEl) wEl.innerHTML = '<span>- ৳ ' + withheld + '</span> <span class="doc-fee-pill">-20%</span>';
  if (nEl) nEl.textContent = '৳ ' + net;

  modal.style.display = 'flex';
}

function closeConsultationFeeModal() {
  const modal = document.getElementById('docConsultationFeeModal');
  if (modal) {
    modal.style.display = 'none';
  }
}

window.doctorEarningsStore = doctorEarningsStore;
window.renderDoctorEarnings = renderDoctorEarnings;
window.openDoctorStatementModal = openDoctorStatementModal;
window.closeDoctorStatementModal = closeDoctorStatementModal;
window.openConsultationFeeDetailModal = openConsultationFeeDetailModal;
window.closeConsultationFeeModal = closeConsultationFeeModal;

function switchDoctorTab(tabIdx) {`;

if (html.includes(jsTarget)) {
  html = html.replace(jsTarget, jsAddition);
  console.log("✓ Added doctor earnings JS controller functions");
} else {
  console.error("✗ Could not find jsTarget");
}

// 6. Hook renderDoctorEarnings into switchDoctorTab(3)
const tabHookTarget = `if (tabIdx === 5) {
    if (typeof renderDoctorMobileQnaList === 'function') {
      renderDoctorMobileQnaList();
    }
  }`;

const tabHookAddition = `if (tabIdx === 3) {
    if (typeof renderDoctorEarnings === 'function') {
      renderDoctorEarnings();
    }
  }
  if (tabIdx === 5) {
    if (typeof renderDoctorMobileQnaList === 'function') {
      renderDoctorMobileQnaList();
    }
  }`;

if (html.includes(tabHookTarget)) {
  html = html.replace(tabHookTarget, tabHookAddition);
  console.log("✓ Hooked renderDoctorEarnings into switchDoctorTab(3)");
} else {
  console.error("✗ Could not find tabHookTarget");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully! New file size:", html.length);
