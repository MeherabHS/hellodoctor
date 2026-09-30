const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original file size:", html.length);

// 1. Add adminNav-finance to sidebar
const navItemHtml = `
        <div class="admin-nav-item" id="adminNav-finance" onclick="switchAdminPage('finance')">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
          <span>All Payments &amp; Escrow</span>
          <span class="admin-nav-badge green" style="background:#DCFCE7; color:#166534; font-weight:800;">Ledger</span>
        </div>`;

if (!html.includes('id="adminNav-finance"')) {
  const targetNav = 'id="adminNav-doctors" onclick="switchAdminPage(\'doctors\')"';
  // Insert right after the doctors nav item
  const docNavEnd = html.indexOf('</div>', html.indexOf(targetNav));
  if (docNavEnd !== -1) {
    html = html.substring(0, docNavEnd + 6) + '\n' + navItemHtml + html.substring(docNavEnd + 6);
    console.log("✓ Added adminNav-finance to sidebar");
  }
}

// 2. Add quick navigation button in adminPage_doctors header
const quickNavBtn = `
            <button class="admin-action-btn" onclick="switchAdminPage('finance')" style="background:#F1F5F9; border:1px solid #CBD5E1; color:#1E293B; font-weight:700; font-size:12px; padding:8px 14px; border-radius:9px; display:inline-flex; align-items:center; gap:6px; cursor:pointer; margin-right:8px;">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
              <span>Master Payment Ledger →</span>
            </button>`;

if (!html.includes('Master Payment Ledger →')) {
  const batchBtnIdx = html.indexOf('<button class="admin-batch-payout-btn" onclick="executeAdminBatchPayout()">');
  if (batchBtnIdx !== -1) {
    html = html.substring(0, batchBtnIdx) + quickNavBtn + '\n            ' + html.substring(batchBtnIdx);
    console.log("✓ Added quick nav button to doctor page header");
  }
}

// 3. Add #adminPage_finance container after #adminPage_grievances
const adminPageFinanceHtml = `
        <!-- ═════════════════════════════════════════════════════════════
             PAGE 9: OMNICHANNEL PAYMENTS & ESCROW MASTER LEDGER
             ═════════════════════════════════════════════════════════════ -->
        <div class="admin-page-view" id="adminPage_finance" style="display:none; flex-direction:column; gap:20px;">
          <!-- Page Header Row -->
          <div style="display:flex; justify-content:space-between; align-items:center; flex-wrap:wrap; gap:12px;">
            <div>
              <h2 style="font-size:20px; font-weight:850; color:#0F172A; margin:0; letter-spacing:-0.4px;">Omnichannel Payment &amp; Escrow Master Ledger</h2>
              <div style="font-size:12px; color:#64748B; margin-top:2px;">Centralized financial audit tracking all patient inflows, gateway escrow locks, 15-20% platform commission cuts, doctor payouts &amp; refund reversals</div>
            </div>

            <div style="display:flex; align-items:center; gap:8px;">
              <button class="admin-action-btn" onclick="switchAdminPage('doctors')" style="background:#F8FAFC; border:1px solid #E2E8F0; color:#334155; font-weight:700; font-size:12px; padding:8px 14px; border-radius:8px; display:inline-flex; align-items:center; gap:6px; cursor:pointer;">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v12M15 9.5a3.5 3.5 0 0 0-7 0c0 2 1.5 3 3.5 3s3.5 1 3.5 3a3.5 3.5 0 0 1-7 0"/></svg>
                <span>Doctor Payouts Hub →</span>
              </button>

              <button class="admin-action-btn" onclick="reconcileAdminMfsWebhooks()" style="background:#EFF6FF; border:1px solid #BFDBFE; color:#1D4ED8; font-weight:700; font-size:12px; padding:8px 14px; border-radius:8px; display:inline-flex; align-items:center; gap:6px; cursor:pointer;">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg>
                <span>Reconcile MFS Webhooks</span>
              </button>

              <button class="admin-action-btn" onclick="exportAdminFinanceLedger()" style="background:#0F172A; border:1px solid #0F172A; color:#FFFFFF; font-weight:700; font-size:12px; padding:8px 14px; border-radius:8px; display:inline-flex; align-items:center; gap:6px; cursor:pointer;">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                <span>Export Ledger CSV</span>
              </button>
            </div>
          </div>

          <!-- 4 Financial Metric KPI Cards -->
          <div class="admin-kpi-grid">
            <!-- Card 1: Gross Platform GMV Inflow -->
            <div class="admin-kpi-card">
              <div class="kpi-top-row">
                <span class="admin-kpi-sub">Total Platform GMV Inflow</span>
                <div class="admin-kpi-icon-circle green" style="width:34px; height:34px; border-radius:10px;">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
                </div>
              </div>
              <div class="admin-kpi-val" id="afKpiGmv">৳ 3,245,000</div>
              <span class="admin-kpi-delta up">↗ +18.4% MoM • 1,864 Transactions</span>
            </div>

            <!-- Card 2: Active Escrow Locked -->
            <div class="admin-kpi-card">
              <div class="kpi-top-row">
                <span class="admin-kpi-sub">Active Escrow Locked</span>
                <div class="admin-kpi-icon-circle blue" style="width:34px; height:34px; border-radius:10px; background:#EFF6FF; color:#2563EB;">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
                </div>
              </div>
              <div class="admin-kpi-val" style="color:#2563EB;" id="afKpiEscrow">৳ 890,800</div>
              <span class="admin-kpi-delta" style="color:#64748B;">Held securely pending doctor sign-off</span>
            </div>

            <!-- Card 3: Platform Commission Retained -->
            <div class="admin-kpi-card">
              <div class="kpi-top-row">
                <span class="admin-kpi-sub">HeloDoc 15-20% Commission</span>
                <div class="admin-kpi-icon-circle green" style="width:34px; height:34px; border-radius:10px; background:#ECFDF5; color:#059669;">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="1" x2="12" y2="23"/><path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"/></svg>
                </div>
              </div>
              <div class="admin-kpi-val" style="color:#059669;" id="afKpiCommission">৳ 568,400</div>
              <span class="admin-kpi-delta up">↗ Net Platform Operating Revenue</span>
            </div>

            <!-- Card 4: Total Disbursements & Refunds -->
            <div class="admin-kpi-card">
              <div class="kpi-top-row">
                <span class="admin-kpi-sub">Total Disbursed Outflows</span>
                <div class="admin-kpi-icon-circle purple" style="width:34px; height:34px; border-radius:10px; background:#F5F3FF; color:#7C3AED;">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="13 17 18 12 13 7"/><polyline points="6 17 11 12 6 7"/></svg>
                </div>
              </div>
              <div class="admin-kpi-val" id="afKpiDisbursed">৳ 2,676,600</div>
              <span class="admin-kpi-delta" style="color:#64748B;">Doctor Wallets + Disputed Refunds</span>
            </div>
          </div>

          <!-- Master Table Card -->
          <div class="admin-card">
            <div class="admin-card-header" style="flex-wrap:wrap; gap:12px;">
              <div>
                <div class="admin-card-title">Omnichannel Financial Audit Table</div>
                <div class="admin-card-subtitle">Complete ledger of MFS gateway checkouts, fee splits, doctor wallets, and clinical escrow holds</div>
              </div>

              <!-- Search & Quick Filters -->
              <div style="display:flex; align-items:center; gap:10px; flex-wrap:wrap;">
                <!-- Searchbar -->
                <div style="position:relative; width:260px;">
                  <input type="text" id="adminFinanceSearchInput" placeholder="Search TxID, Patient, Doctor, Ref..." oninput="filterAdminFinanceTable()" style="width:100%; padding:7px 10px 7px 32px; border:1px solid #CBD5E1; border-radius:8px; font-size:12px; outline:none; background:#FFFFFF;">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" stroke-width="2" style="position:absolute; left:10px; top:50%; transform:translateY(-50%); pointer-events:none;"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
                </div>

                <!-- Gateway Dropdown -->
                <select id="adminFinanceGatewaySelect" onchange="filterAdminFinanceTable()" style="padding:7px 10px; border:1px solid #CBD5E1; border-radius:8px; font-size:12px; background:#FFFFFF; color:#334155; outline:none;">
                  <option value="ALL">All Gateways</option>
                  <option value="bKash">bKash Merchant Pay</option>
                  <option value="Nagad">Nagad Business Pay</option>
                  <option value="Card">Visa / Mastercard (SSLCommerz)</option>
                  <option value="MFS Bulk API">MFS Bulk Payout API</option>
                </select>

                <!-- Filter Pills -->
                <div style="display:flex; align-items:center; gap:4px; background:#F1F5F9; padding:3px; border-radius:8px;">
                  <button class="admin-filter-pill active" id="btnFinFilterAll" onclick="setAdminFinanceTypeFilter('ALL', this)" style="padding:4px 10px; border-radius:6px; font-size:11px; font-weight:700; border:none; cursor:pointer;">All</button>
                  <button class="admin-filter-pill" id="btnFinFilterInflow" onclick="setAdminFinanceTypeFilter('INFLOW', this)" style="padding:4px 10px; border-radius:6px; font-size:11px; font-weight:700; border:none; cursor:pointer; background:transparent; color:#64748B;">Inflow 📥</button>
                  <button class="admin-filter-pill" id="btnFinFilterEscrow" onclick="setAdminFinanceTypeFilter('ESCROW', this)" style="padding:4px 10px; border-radius:6px; font-size:11px; font-weight:700; border:none; cursor:pointer; background:transparent; color:#64748B;">Escrow 🔒</button>
                  <button class="admin-filter-pill" id="btnFinFilterPayout" onclick="setAdminFinanceTypeFilter('PAYOUT', this)" style="padding:4px 10px; border-radius:6px; font-size:11px; font-weight:700; border:none; cursor:pointer; background:transparent; color:#64748B;">Payout 📤</button>
                  <button class="admin-filter-pill" id="btnFinFilterRefund" onclick="setAdminFinanceTypeFilter('REFUND', this)" style="padding:4px 10px; border-radius:6px; font-size:11px; font-weight:700; border:none; cursor:pointer; background:transparent; color:#64748B;">Refund ↩️</button>
                </div>
              </div>
            </div>

            <!-- Ledger Table -->
            <div style="overflow-x:auto;">
              <table class="admin-table" id="adminFinanceTable">
                <thead>
                  <tr>
                    <th>TxID &amp; MFS Ref</th>
                    <th>Timestamp</th>
                    <th>Channel &amp; Type</th>
                    <th>Consultation / Case</th>
                    <th>Parties (Patient / Doctor)</th>
                    <th style="text-align:right;">Gross (৳)</th>
                    <th style="text-align:right;">HeloDoc Cut</th>
                    <th style="text-align:right;">Net (৳)</th>
                    <th>Gateway &amp; Escrow State</th>
                    <th style="text-align:center;">Action</th>
                  </tr>
                </thead>
                <tbody id="adminFinanceTableBody">
                  <!-- Rendered dynamically via renderAdminFinanceTable() -->
                </tbody>
              </table>
            </div>

            <div style="padding:12px 18px; border-top:1px solid #E2E8F0; display:flex; justify-content:space-between; align-items:center; background:#F8FAFC; font-size:11.5px; color:#64748B;">
              <div>Showing <b id="afVisibleCount" style="color:#0F172A;">8</b> of <b id="afTotalCount" style="color:#0F172A;">8</b> transactions • Real-time Webhook Ingestion Active</div>
              <div style="display:flex; align-items:center; gap:8px;">
                <span style="display:inline-block; width:8px; height:8px; border-radius:50%; background:#10B981;"></span>
                <span style="font-weight:700; color:#047857;">MFS Webhooks Active (0% dropped)</span>
              </div>
            </div>
          </div>
        </div><!-- /#adminPage_finance -->`;

if (!html.includes('id="adminPage_finance"')) {
  const grvPageEnd = html.indexOf('</div><!-- /#adminPage_grievances -->');
  if (grvPageEnd !== -1) {
    html = html.substring(0, grvPageEnd + 37) + '\n' + adminPageFinanceHtml + html.substring(grvPageEnd + 37);
    console.log("✓ Added adminPage_finance container");
  }
}

// 4. Add #adminTransactionDetailModal modal right before <!-- GLOBAL LOADING TRANSITION OVERLAY
const transactionModalHtml = `
  <!-- ═════════════════════════════════════════════════════════════
       MODAL: TRANSACTION AUDIT & FORENSIC RECEIPT (CENTRAL ADMIN)
       ═════════════════════════════════════════════════════════════ -->
  <div class="admin-modal-backdrop" id="adminTransactionDetailModal" style="display:none;" onclick="if(event.target===this) closeAdminTransactionModal();">
    <div class="admin-modal-card" style="max-width:760px; width:95%;">
      <!-- Modal Header -->
      <div style="background:#0F172A; color:#fff; padding:18px 22px; display:flex; justify-content:space-between; align-items:center;">
        <div style="display:flex; align-items:center; gap:12px;">
          <div style="width:42px; height:42px; border-radius:10px; background:rgba(255,255,255,0.1); display:grid; place-items:center; color:#38BDF8;">
            <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/></svg>
          </div>
          <div>
            <div style="display:flex; align-items:center; gap:8px;">
              <span id="atdTxId" style="font-size:16px; font-weight:800; letter-spacing:-0.2px;">TXN-BK-94812</span>
              <span class="badge badge-green" id="atdStatusBadge">Settled ✓</span>
              <span class="payout-method-badge bkash" id="atdGatewayBadge" style="margin:0;">bKash Merchant</span>
            </div>
            <div id="atdTimestamp" style="font-size:11.5px; color:#94A3B8; margin-top:2px;">Today, 10:28 AM • Gateway TrxID: BK-MER-88492019</div>
          </div>
        </div>
        <button onclick="closeAdminTransactionModal()" style="background:transparent; border:none; color:#94A3B8; font-size:20px; cursor:pointer;">✕</button>
      </div>

      <!-- 3-Way Reconciliation Breakdown Banner -->
      <div style="display:grid; grid-template-columns:repeat(3, 1fr); gap:1px; background:#E2E8F0; border-bottom:1px solid #E2E8F0;">
        <div style="background:#FFFFFF; padding:14px 18px;">
          <div style="font-size:10px; text-transform:uppercase; font-weight:800; color:#64748B;">Gross Inflow (Patient Fee)</div>
          <div style="font-size:20px; font-weight:900; color:#0F172A; margin-top:4px;" id="atdGrossAmount">৳ 800</div>
          <div style="font-size:10.5px; color:#10B981; font-weight:700;">Debited from MFS Wallet</div>
        </div>
        <div style="background:#FFFFFF; padding:14px 18px;">
          <div style="font-size:10px; text-transform:uppercase; font-weight:800; color:#64748B;">HeloDoc Platform Fee (15-20%)</div>
          <div style="font-size:20px; font-weight:900; color:#059669; margin-top:4px;" id="atdPlatformFee">৳ 120 (15%)</div>
          <div style="font-size:10.5px; color:#64748B;">Platform Infrastructure Cut</div>
        </div>
        <div style="background:#FFFFFF; padding:14px 18px;">
          <div style="font-size:10px; text-transform:uppercase; font-weight:800; color:#64748B;">Doctor Net / Patient Refund</div>
          <div style="font-size:20px; font-weight:900; color:#2563EB; margin-top:4px;" id="atdNetAmount">৳ 680 (85%)</div>
          <div style="font-size:10.5px; color:#64748B;" id="atdNetNote">Payable to Physician Wallet</div>
        </div>
      </div>

      <!-- Modal Body -->
      <div style="padding:20px 22px; display:flex; flex-direction:column; gap:16px; max-height:60vh; overflow-y:auto; background:#FAFAFA;">
        <!-- Two Column Metadata -->
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px;">
          <!-- Patient Box -->
          <div style="background:#FFFFFF; border:1px solid #E2E8F0; border-radius:10px; padding:14px;">
            <div style="font-size:11px; font-weight:800; text-transform:uppercase; color:#64748B; margin-bottom:8px;">Payer (Patient) Profile</div>
            <div style="display:flex; align-items:center; gap:10px;">
              <div id="atdPatientAvatar" style="width:36px; height:36px; border-radius:50%; background:#2563EB; color:#fff; display:grid; place-items:center; font-weight:800; font-size:12px;">SK</div>
              <div>
                <strong id="atdPatientName" style="font-size:13px; color:#0F172A;">Sarah Khan</strong>
                <div id="atdPatientPhone" style="font-size:11px; color:#64748B;">+880 1711-234567</div>
              </div>
            </div>
            <div style="margin-top:10px; padding-top:10px; border-top:1px solid #F1F5F9; font-size:11px; color:#475569;">
              Consultation: <b id="atdConsultationId" style="color:#0F172A;">CONS-9481</b> • <span id="atdSessionType">Video Visit</span>
            </div>
          </div>

          <!-- Doctor Box -->
          <div style="background:#FFFFFF; border:1px solid #E2E8F0; border-radius:10px; padding:14px;">
            <div style="font-size:11px; font-weight:800; text-transform:uppercase; color:#64748B; margin-bottom:8px;">Recipient (Doctor / System)</div>
            <div style="display:flex; align-items:center; gap:10px;">
              <div id="atdDoctorAvatar" style="width:36px; height:36px; border-radius:50%; background:#059669; color:#fff; display:grid; place-items:center; font-weight:800; font-size:12px;">SA</div>
              <div>
                <strong id="atdDoctorName" style="font-size:13px; color:#0F172A;">Dr. Sabrina Akter</strong>
                <div id="atdDoctorBmdc" style="font-size:11px; color:#64748B;">BMDC #45821 • Internal Medicine</div>
              </div>
            </div>
            <div style="margin-top:10px; padding-top:10px; border-top:1px solid #F1F5F9; font-size:11px; color:#475569;">
              Escrow Governance: <b id="atdEscrowStatus" style="color:#D97706;">Locked in Escrow Gateway</b>
            </div>
          </div>
        </div>

        <!-- MFS Webhook Payload Box -->
        <div style="background:#FFFFFF; border:1px solid #E2E8F0; border-radius:10px; padding:14px;">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
            <div style="font-size:11px; font-weight:800; text-transform:uppercase; color:#64748B;">MFS Gateway Telemetry &amp; Webhook Signature</div>
            <span class="badge badge-green" style="font-size:9.5px;">Webhook Verified ✓</span>
          </div>
          <div style="background:#0F172A; color:#E2E8F0; padding:12px; border-radius:8px; font-family:monospace; font-size:11px; overflow-x:auto;" id="atdGatewayJson">
            <!-- Raw JSON Webhook -->
          </div>
        </div>

        <!-- Chronological Financial Event Trail -->
        <div style="background:#FFFFFF; border:1px solid #E2E8F0; border-radius:10px; padding:14px;">
          <div style="font-size:11px; font-weight:800; text-transform:uppercase; color:#64748B; margin-bottom:10px;">Audit &amp; Reconciliation Timeline</div>
          <div id="atdTimelineList" style="display:flex; flex-direction:column; gap:8px;">
            <!-- Timeline items -->
          </div>
        </div>
      </div>

      <!-- Modal Footer -->
      <div style="padding:14px 22px; background:#FFFFFF; border-top:1px solid #E2E8F0; display:flex; justify-content:space-between; align-items:center;">
        <button class="sheet-btn" onclick="showAppToast('Receipt Downloaded 📄', 'Official tax-compliant transaction voucher downloaded.', 'success', '📄')" style="padding:8px 16px; font-size:12px; background:#F1F5F9; border:1px solid #CBD5E1; color:#334155; border-radius:8px; cursor:pointer;">
          📄 Print / Save Tax Voucher
        </button>

        <button class="sheet-btn primary" onclick="closeAdminTransactionModal()" style="padding:8px 20px; font-size:12px; background:#0F172A; color:#fff; border-radius:8px; cursor:pointer;">
          Close Audit Window
        </button>
      </div>
    </div>
  </div><!-- /#adminTransactionDetailModal -->`;

if (!html.includes('id="adminTransactionDetailModal"')) {
  const loadingOverlayIdx = html.indexOf('<!-- ═════════════════════════════════════════════════════════════\n       GLOBAL LOADING TRANSITION OVERLAY');
  if (loadingOverlayIdx !== -1) {
    html = html.substring(0, loadingOverlayIdx) + transactionModalHtml + '\n  ' + html.substring(loadingOverlayIdx);
    console.log("✓ Added adminTransactionDetailModal modal");
  }
}

// 5. Update adminPageMeta in script
const oldMetaAnchor = "const adminPageMeta = {";
if (html.includes(oldMetaAnchor) && !html.includes("finance: {")) {
  const financeMetaChunk = `        finance: {
          title: 'Omnichannel Payment & Escrow Master Ledger',
          breadcrumb: 'Real-time bKash, Nagad & Card Inflows • 15-20% Platform Commission Splits • Escrow Release Audit'
        },
`;
  html = html.replace(oldMetaAnchor + '\n', oldMetaAnchor + '\n' + financeMetaChunk);
  console.log("✓ Updated adminPageMeta with finance meta");
}

// 6. Update switchAdminPage in script to render finance table
const switchTarget = "if (pageId === 'grievances' && typeof renderAdminGrievancesTable === 'function') {\n          renderAdminGrievancesTable();\n        }";
const switchAdd = `
        if (pageId === 'finance' && typeof renderAdminFinanceTable === 'function') {
          renderAdminFinanceTable();
        }`;
if (html.includes(switchTarget) && !html.includes("pageId === 'finance'")) {
  html = html.replace(switchTarget, switchTarget + switchAdd);
  console.log("✓ Updated switchAdminPage for finance");
}

// 7. Update handleAdminGlobalSearch in script for finance
const searchTarget = "if (grvInput) grvInput.value = query;\n          filterAdminGrievances();\n        }";
const searchAdd = ` else if (activeAdminPage === 'finance') {
          const finInput = document.getElementById('adminFinanceSearchInput');
          if (finInput) finInput.value = query;
          filterAdminFinanceTable();
        }`;
if (html.includes(searchTarget) && !html.includes("activeAdminPage === 'finance'")) {
  html = html.replace(searchTarget, searchTarget + searchAdd);
  console.log("✓ Updated handleAdminGlobalSearch for finance");
}

// 8. Add JavaScript Finance Data Store and Controllers before </script>
const financeJsCode = `
      // ═════════════════════════════════════════════════════════════
      // CENTRAL ADMIN: OMNICHANNEL PAYMENTS & ESCROW LEDGER STORE
      // ═════════════════════════════════════════════════════════════
      let activeAdminFinanceTypeFilter = 'ALL';

      const adminTransactionStore = [
        {
          txId: 'TXN-BK-94812',
          gatewayRef: 'BK-MER-88492019',
          timestamp: 'Today, 10:28 AM',
          consultationId: 'CONS-9481',
          sessionType: 'Video Consultation (10 min)',
          type: 'ESCROW',
          typeLabel: 'Consultation Inflow (Escrow)',
          gateway: 'bKash',
          gatewayLabel: 'bKash Merchant',
          patient: { name: 'Sarah Khan', phone: '+880 1711-234567', id: 'USR-8921', avatar: 'SK', avatarBg: '#2563EB' },
          doctor: { name: 'Dr. Sabrina Akter', bmdc: 'BMDC #45821', specialty: 'Internal Medicine', avatar: 'SA', avatarBg: '#059669' },
          grossAmount: 800,
          platformFeePercent: 15,
          platformFeeAmount: 120,
          netAmount: 680,
          direction: 'INFLOW',
          escrowStatus: 'ESCROW_LOCKED',
          escrowStatusLabel: 'Locked in Escrow 🔒',
          statusBadgeCls: 'badge-amber',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CONS-9481',
            paymentExecuteTime: '2026-09-30T10:28:14.218+06:00',
            payerReference: '01711234567',
            trxID: 'BK-MER-88492019',
            merchantAccount: '01713-HELODOC-MERCHANT',
            transactionStatus: 'Completed',
            signature: 'HMAC_SHA256_VERIFIED_7a9c1e'
          },
          auditTrail: [
            { time: '10:28:02', event: 'Payment initiated via bKash Checkout URL' },
            { time: '10:28:14', event: 'bKash Webhook Callback 200 OK (TrxID: BK-MER-88492019)' },
            { time: '10:28:15', event: 'HeloDoc Escrow Lock Activated (৳800 held in central vault)' },
            { time: '10:32:14', event: 'Video call ended prematurely (2m 14s). Grievance GRV-20260930-01 logged' },
            { time: '10:32:15', event: 'Automatic Escrow Disbursement Frozen pending Medical Board Adjudication' }
          ]
        },
        {
          txId: 'TXN-NG-77124',
          gatewayRef: 'NG-TXN-4921004',
          timestamp: 'Today, 09:45 AM',
          consultationId: 'CONS-9482',
          sessionType: 'Pediatric Surgery Review (15 min)',
          type: 'INFLOW',
          typeLabel: 'Consultation Inflow',
          gateway: 'Nagad',
          gatewayLabel: 'Nagad Business',
          patient: { name: 'Rafiq Ahmed', phone: '+880 1712-345678', id: 'USR-8922', avatar: 'RA', avatarBg: '#1D4ED8' },
          doctor: { name: 'Dr. Sadik Al-Amin', bmdc: 'BMDC #A-68192', specialty: 'Pediatric Surgery', avatar: 'SA', avatarBg: '#D97706' },
          grossAmount: 1200,
          platformFeePercent: 15,
          platformFeeAmount: 180,
          netAmount: 1020,
          direction: 'INFLOW',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Settled ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CONS-9482',
            paymentExecuteTime: '2026-09-30T09:45:08.102+06:00',
            payerReference: '01712345678',
            trxID: 'NG-TXN-4921004',
            merchantAccount: '01844-HELODOC-NAGAD',
            transactionStatus: 'Completed',
            signature: 'HMAC_SHA256_VERIFIED_882b01'
          },
          auditTrail: [
            { time: '09:44:50', event: 'Nagad Direct Checkout token granted' },
            { time: '09:45:08', event: 'Payment confirmed by Nagad Webhook' },
            { time: '09:45:10', event: 'Held in Escrow during 15m consultation' },
            { time: '10:00:22', event: 'Consultation completed & signed off. Escrow released to physician wallet.' }
          ]
        },
        {
          txId: 'TXN-BK-88291',
          gatewayRef: 'BK-MER-3918402',
          timestamp: 'Today, 09:12 AM',
          consultationId: 'CHAT-4819',
          sessionType: '24h Continuous Chat Subscription',
          type: 'INFLOW',
          typeLabel: '24h Chat Subscription',
          gateway: 'bKash',
          gatewayLabel: 'bKash Merchant',
          patient: { name: 'Nusrat Jahan', phone: '+880 1819-456789', id: 'USR-8923', avatar: 'NJ', avatarBg: '#9333EA' },
          doctor: { name: 'Dr. Anika Tahsin', bmdc: 'BMDC #A-59124', specialty: 'Dermatology Consultant', avatar: 'AT', avatarBg: '#EC4899' },
          grossAmount: 300,
          platformFeePercent: 20,
          platformFeeAmount: 60,
          netAmount: 240,
          direction: 'INFLOW',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Settled ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CHAT-4819',
            paymentExecuteTime: '2026-09-30T09:12:30.540+06:00',
            payerReference: '01819456789',
            trxID: 'BK-MER-3918402',
            merchantAccount: '01713-HELODOC-MERCHANT',
            transactionStatus: 'Completed',
            signature: 'HMAC_SHA256_VERIFIED_4f810c'
          },
          auditTrail: [
            { time: '09:12:10', event: 'Chat checkout triggered' },
            { time: '09:12:30', event: 'Instant bKash debit confirmed' },
            { time: '09:12:31', event: '24h timer initialized. 20% platform cut retained, ৳240 credited to doctor ledger.' }
          ]
        },
        {
          txId: 'TXN-RF-91823',
          gatewayRef: 'BK-REF-91823',
          timestamp: 'Today, 08:35 AM',
          consultationId: 'CONS-9390',
          sessionType: 'Escrow Dispute Refund',
          type: 'REFUND',
          typeLabel: 'Patient Grievance Refund',
          gateway: 'bKash',
          gatewayLabel: 'bKash Instant Refund',
          patient: { name: 'Tanvir Hasan', phone: '+880 1612-987654', id: 'USR-8924', avatar: 'TH', avatarBg: '#DC2626' },
          doctor: { name: 'Central Escrow Vault', bmdc: 'SYSTEM-REVERSAL', specialty: 'Platform Reconciliation', avatar: 'HD', avatarBg: '#0F172A' },
          grossAmount: 800,
          platformFeePercent: 0,
          platformFeeAmount: 0,
          netAmount: 800,
          direction: 'REFUND',
          escrowStatus: 'REFUNDED',
          escrowStatusLabel: 'Refunded ↩️',
          statusBadgeCls: 'badge-rose',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-REF-CONS-9390',
            paymentExecuteTime: '2026-09-30T08:35:19.004+06:00',
            payerReference: '01612987654',
            trxID: 'BK-REF-91823',
            merchantAccount: '01713-HELODOC-MERCHANT',
            transactionStatus: 'Refunded',
            signature: 'HMAC_SHA256_VERIFIED_99c31a'
          },
          auditTrail: [
            { time: '08:30:00', event: 'Patient lodged grievance for dropped call (38s duration)' },
            { time: '08:34:40', event: 'Medical Board confirmed premature disconnect; ordered 100% refund' },
            { time: '08:35:19', event: 'Instant bKash wallet reversal executed. TrxID: BK-REF-91823' }
          ]
        },
        {
          txId: 'TXN-CD-66120',
          gatewayRef: 'SSL-CARD-773194',
          timestamp: 'Today, 08:10 AM',
          consultationId: 'CONS-9388',
          sessionType: 'Video Visit (10 min)',
          type: 'INFLOW',
          typeLabel: 'Consultation Inflow',
          gateway: 'Card',
          gatewayLabel: 'Visa / Mastercard (SSLCommerz)',
          patient: { name: 'Kamal Hossain', phone: '+880 1912-345678', id: 'USR-8925', avatar: 'KH', avatarBg: '#047857' },
          doctor: { name: 'Dr. Farhana Yeasmin', bmdc: 'BMDC #A-71203', specialty: 'Obstetrics & Gynecology', avatar: 'FY', avatarBg: '#7C3AED' },
          grossAmount: 1000,
          platformFeePercent: 15,
          platformFeeAmount: 150,
          netAmount: 850,
          direction: 'INFLOW',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Settled ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CONS-9388',
            paymentExecuteTime: '2026-09-30T08:10:44.811+06:00',
            payerReference: 'CARD-411122******1234',
            trxID: 'SSL-CARD-773194',
            merchantAccount: 'SSLCOMMERZ-HELODOC-LIVE',
            transactionStatus: 'VALIDATED',
            signature: 'SSL_HASH_VERIFIED_331b2'
          },
          auditTrail: [
            { time: '08:10:02', event: 'SSLCommerz payment gateway validated' },
            { time: '08:10:44', event: 'IPN Webhook received & 3D-Secure verified' },
            { time: '08:25:00', event: 'Consultation completed. ৳850 settled to Dr. Farhana wallet.' }
          ]
        },
        {
          txId: 'TXN-PO-99411',
          gatewayRef: 'BULK-MFS-99120',
          timestamp: 'Yesterday, 11:59 PM',
          consultationId: 'BATCH-PO-20260929',
          sessionType: 'Physician Wallet Daily Settlement',
          type: 'PAYOUT',
          typeLabel: 'Doctor Batch Payout',
          gateway: 'MFS Bulk API',
          gatewayLabel: 'bKash + Nagad Bulk API',
          patient: { name: 'HeloDoc Treasury', phone: 'SYSTEM', id: 'TREASURY-01', avatar: 'HD', avatarBg: '#0F172A' },
          doctor: { name: 'Active Physicians (18 Doctors)', bmdc: 'MULTI-RECON', specialty: 'Batch Settlement', avatar: 'DR', avatarBg: '#2563EB' },
          grossAmount: 156825,
          platformFeePercent: 0,
          platformFeeAmount: 0,
          netAmount: 156825,
          direction: 'PAYOUT',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Disbursed ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'BATCH-SETTLE-20260929',
            paymentExecuteTime: '2026-09-29T23:59:00.000+06:00',
            payerReference: 'HELODOC-TREASURY',
            trxID: 'BULK-MFS-99120',
            merchantAccount: 'MFS-DISBURSEMENT-CORP',
            transactionStatus: 'PROCESSED',
            signature: 'MFS_BULK_HASH_991823'
          },
          auditTrail: [
            { time: '23:58:30', event: 'Daily batch settlement compiled for 18 eligible doctors' },
            { time: '23:59:00', event: 'Disbursement sent via bKash & Nagad Corporate APIs' },
            { time: '23:59:12', event: 'All 18 physician wallets credited. Escrow ledger cleared.' }
          ]
        },
        {
          txId: 'TXN-BK-94815',
          gatewayRef: 'BK-MER-7718290',
          timestamp: 'Yesterday, 06:20 PM',
          consultationId: 'CONS-9340',
          sessionType: 'Video Consultation (10 min)',
          type: 'INFLOW',
          typeLabel: 'Consultation Inflow',
          gateway: 'bKash',
          gatewayLabel: 'bKash Merchant',
          patient: { name: 'Farzana Haque', phone: '+880 1714-567890', id: 'USR-8926', avatar: 'FH', avatarBg: '#0891B2' },
          doctor: { name: 'Dr. Sabrina Akter', bmdc: 'BMDC #45821', specialty: 'Internal Medicine', avatar: 'SA', avatarBg: '#059669' },
          grossAmount: 800,
          platformFeePercent: 15,
          platformFeeAmount: 120,
          netAmount: 680,
          direction: 'INFLOW',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Settled ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CONS-9340',
            paymentExecuteTime: '2026-09-29T18:20:12.441+06:00',
            payerReference: '01714567890',
            trxID: 'BK-MER-7718290',
            merchantAccount: '01713-HELODOC-MERCHANT',
            transactionStatus: 'Completed',
            signature: 'HMAC_SHA256_VERIFIED_11a84c'
          },
          auditTrail: [
            { time: '18:20:00', event: 'bKash Merchant Pay verified' },
            { time: '18:20:12', event: 'Funds held in Escrow' },
            { time: '18:31:00', event: '10m call concluded normally with e-Rx #Rx-084 issued. Escrow settled.' }
          ]
        },
        {
          txId: 'TXN-NG-55201',
          gatewayRef: 'NG-TXN-102948',
          timestamp: 'Yesterday, 04:15 PM',
          consultationId: 'CONS-9321',
          sessionType: 'General Health Consult (10 min)',
          type: 'INFLOW',
          typeLabel: 'Consultation Inflow',
          gateway: 'Nagad',
          gatewayLabel: 'Nagad Business',
          patient: { name: 'Sarah Ahmed', phone: '+880 1711-234567', id: 'USR-6521', avatar: 'SA', avatarBg: '#BE185D' },
          doctor: { name: 'Dr. Kamal Uddin', bmdc: 'BMDC #A-39182', specialty: 'General Practice', avatar: 'KU', avatarBg: '#059669' },
          grossAmount: 600,
          platformFeePercent: 15,
          platformFeeAmount: 90,
          netAmount: 510,
          direction: 'INFLOW',
          escrowStatus: 'SETTLED',
          escrowStatusLabel: 'Settled ✓',
          statusBadgeCls: 'badge-green',
          gatewayPayload: {
            merchantInvoiceNumber: 'INV-2026-CONS-9321',
            paymentExecuteTime: '2026-09-29T16:15:33.201+06:00',
            payerReference: '01711234567',
            trxID: 'NG-TXN-102948',
            merchantAccount: '01844-HELODOC-NAGAD',
            transactionStatus: 'Completed',
            signature: 'HMAC_SHA256_VERIFIED_08c44e'
          },
          auditTrail: [
            { time: '16:15:10', event: 'Nagad Business token executed' },
            { time: '16:15:33', event: 'Escrow lock active' },
            { time: '16:26:00', event: '10m consultation completed. ৳510 net paid to Dr. Kamal.' }
          ]
        }
      ];

      function renderAdminFinanceTable(listToRender = null) {
        const list = listToRender || adminTransactionStore;
        const tbody = document.getElementById('adminFinanceTableBody');
        if (!tbody) return;

        if (list.length === 0) {
          tbody.innerHTML = '<tr><td colspan="10" style="text-align:center; padding:32px; color:#64748B;">No transactions match your search/filter criteria.</td></tr>';
          const visEl = document.getElementById('afVisibleCount');
          if (visEl) visEl.textContent = '0';
          return;
        }

        tbody.innerHTML = list.map(tx => {
          let gatewayBadgeCls = 'payout-method-badge';
          if (tx.gateway === 'bKash') gatewayBadgeCls += ' bkash';
          else if (tx.gateway === 'Nagad') gatewayBadgeCls += ' nagad';
          else gatewayBadgeCls += ' bank';

          return \`
            <tr class="admin-clickable-row" onclick="openAdminTransactionModal('\${tx.txId}')">
              <td>
                <div style="font-weight:800; color:#0F172A; font-size:12.5px;">\${tx.txId}</div>
                <div style="font-size:10px; color:#64748B; font-family:monospace;">\${tx.gatewayRef}</div>
              </td>
              <td style="font-size:11.5px; color:#334155; white-space:nowrap;">\${tx.timestamp}</td>
              <td>
                <span class="\${gatewayBadgeCls}" style="margin-bottom:2px;">
                  \${tx.gateway === 'bKash' ? '<svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>' : ''}
                  <span>\${tx.gatewayLabel}</span>
                </span>
                <div style="font-size:10px; color:#64748B; margin-top:2px;">\${tx.typeLabel}</div>
              </td>
              <td>
                <b style="color:#2563EB; font-size:12px;">\${tx.consultationId}</b>
                <div style="font-size:10px; color:#64748B;">\${tx.sessionType}</div>
              </td>
              <td>
                <div style="display:flex; align-items:center; gap:6px;">
                  <span style="font-size:12px; font-weight:700; color:#0F172A;">\${tx.patient.name}</span>
                  <span style="color:#94A3B8;">→</span>
                  <span style="font-size:12px; font-weight:700; color:#0F172A;">\${tx.doctor.name}</span>
                </div>
                <div style="font-size:10px; color:#64748B;">\${tx.patient.phone} • \${tx.doctor.bmdc}</div>
              </td>
              <td style="text-align:right; font-weight:800; color:#0F172A; font-size:13px;">৳ \${tx.grossAmount.toLocaleString()}</td>
              <td style="text-align:right; font-size:12px; color:#059669; font-weight:700;">৳ \${tx.platformFeeAmount.toLocaleString()} <span style="font-size:9.5px; color:#64748B;">(\${tx.platformFeePercent}%)</span></td>
              <td style="text-align:right; font-weight:850; color:#2563EB; font-size:13.5px;">৳ \${tx.netAmount.toLocaleString()}</td>
              <td>
                <span class="badge \${tx.statusBadgeCls}">\${tx.escrowStatusLabel}</span>
              </td>
              <td style="text-align:center;">
                <button onclick="event.stopPropagation(); openAdminTransactionModal('\${tx.txId}');" style="padding:4px 8px; border:1px solid #CBD5E1; background:#FFFFFF; border-radius:6px; font-size:11px; font-weight:700; color:#334155; cursor:pointer;">
                  Receipt 🔍
                </button>
              </td>
            </tr>
          \`;
        }).join('');

        const visEl = document.getElementById('afVisibleCount');
        const totEl = document.getElementById('afTotalCount');
        if (visEl) visEl.textContent = list.length;
        if (totEl) totEl.textContent = adminTransactionStore.length;
      }

      function filterAdminFinanceTable() {
        const query = (document.getElementById('adminFinanceSearchInput')?.value || '').toLowerCase().trim();
        const gatewayFilter = document.getElementById('adminFinanceGatewaySelect')?.value || 'ALL';

        let filtered = adminTransactionStore.filter(tx => {
          // Type filter
          if (activeAdminFinanceTypeFilter === 'INFLOW' && tx.direction !== 'INFLOW') return false;
          if (activeAdminFinanceTypeFilter === 'ESCROW' && tx.escrowStatus !== 'ESCROW_LOCKED') return false;
          if (activeAdminFinanceTypeFilter === 'PAYOUT' && tx.type !== 'PAYOUT') return false;
          if (activeAdminFinanceTypeFilter === 'REFUND' && tx.direction !== 'REFUND') return false;

          // Gateway filter
          if (gatewayFilter !== 'ALL' && tx.gateway !== gatewayFilter) return false;

          // Search query
          if (query) {
            const matchesId = tx.txId.toLowerCase().includes(query) || tx.gatewayRef.toLowerCase().includes(query) || tx.consultationId.toLowerCase().includes(query);
            const matchesPatient = tx.patient.name.toLowerCase().includes(query) || tx.patient.phone.toLowerCase().includes(query);
            const matchesDoctor = tx.doctor.name.toLowerCase().includes(query) || tx.doctor.bmdc.toLowerCase().includes(query);
            return matchesId || matchesPatient || matchesDoctor;
          }

          return true;
        });

        renderAdminFinanceTable(filtered);
      }

      function setAdminFinanceTypeFilter(type, btnEl) {
        activeAdminFinanceTypeFilter = type;
        const container = btnEl.parentElement;
        if (container) {
          container.querySelectorAll('.admin-filter-pill').forEach(btn => {
            btn.classList.remove('active');
            btn.style.background = 'transparent';
            btn.style.color = '#64748B';
          });
          btnEl.classList.add('active');
          btnEl.style.background = '#FFFFFF';
          btnEl.style.color = '#0F172A';
        }
        filterAdminFinanceTable();
      }

      function openAdminTransactionModal(txId) {
        const tx = adminTransactionStore.find(t => t.txId === txId) || adminTransactionStore[0];
        if (!tx) return;

        const modal = document.getElementById('adminTransactionDetailModal');
        if (!modal) return;

        const idEl = document.getElementById('atdTxId');
        const stBadge = document.getElementById('atdStatusBadge');
        const gwBadge = document.getElementById('atdGatewayBadge');
        const tsEl = document.getElementById('atdTimestamp');
        const grEl = document.getElementById('atdGrossAmount');
        const pfEl = document.getElementById('atdPlatformFee');
        const ntEl = document.getElementById('atdNetAmount');
        const nnEl = document.getElementById('atdNetNote');
        const paAv = document.getElementById('atdPatientAvatar');
        const paNm = document.getElementById('atdPatientName');
        const paPh = document.getElementById('atdPatientPhone');
        const cidEl = document.getElementById('atdConsultationId');
        const sType = document.getElementById('atdSessionType');
        const daAv = document.getElementById('atdDoctorAvatar');
        const daNm = document.getElementById('atdDoctorName');
        const daBm = document.getElementById('atdDoctorBmdc');
        const escEl = document.getElementById('atdEscrowStatus');
        const jsonEl = document.getElementById('atdGatewayJson');
        const tlEl = document.getElementById('atdTimelineList');

        if (idEl) idEl.textContent = tx.txId;
        if (stBadge) {
          stBadge.className = 'badge ' + tx.statusBadgeCls;
          stBadge.textContent = tx.escrowStatusLabel;
        }
        if (gwBadge) {
          gwBadge.className = 'payout-method-badge ' + (tx.gateway === 'bKash' ? 'bkash' : (tx.gateway === 'Nagad' ? 'nagad' : 'bank'));
          gwBadge.textContent = tx.gatewayLabel;
        }
        if (tsEl) tsEl.textContent = \`\${tx.timestamp} • Gateway TrxID: \${tx.gatewayRef}\`;
        if (grEl) grEl.textContent = \`৳ \${tx.grossAmount.toLocaleString()}\`;
        if (pfEl) pfEl.textContent = \`৳ \${tx.platformFeeAmount.toLocaleString()} (\${tx.platformFeePercent}%)\`;
        if (ntEl) ntEl.textContent = \`৳ \${tx.netAmount.toLocaleString()}\`;
        if (nnEl) nnEl.textContent = tx.direction === 'REFUND' ? 'Reversed to Patient MFS Wallet' : (tx.type === 'PAYOUT' ? 'Disbursed via Bulk Banking' : 'Payable to Physician Wallet');

        if (paAv) {
          paAv.textContent = tx.patient.avatar;
          paAv.style.background = tx.patient.avatarBg;
        }
        if (paNm) paNm.textContent = tx.patient.name;
        if (paPh) paPh.textContent = tx.patient.phone;
        if (cidEl) cidEl.textContent = tx.consultationId;
        if (sType) sType.textContent = tx.sessionType;

        if (daAv) {
          daAv.textContent = tx.doctor.avatar;
          daAv.style.background = tx.doctor.avatarBg;
        }
        if (daNm) daNm.textContent = tx.doctor.name;
        if (daBm) daBm.textContent = \`\${tx.doctor.bmdc} • \${tx.doctor.specialty}\`;
        if (escEl) {
          escEl.textContent = tx.escrowStatusLabel;
          escEl.style.color = tx.escrowStatus === 'ESCROW_LOCKED' ? '#D97706' : (tx.escrowStatus === 'REFUNDED' ? '#DC2626' : '#059669');
        }

        if (jsonEl) {
          jsonEl.textContent = JSON.stringify(tx.gatewayPayload, null, 2);
        }

        if (tlEl && tx.auditTrail) {
          tlEl.innerHTML = tx.auditTrail.map(t => \`
            <div style="display:flex; align-items:flex-start; gap:8px; font-size:11.5px;">
              <span style="font-family:monospace; color:#2563EB; font-weight:700; min-width:60px;">\${t.time}</span>
              <span style="color:#334155;">\${t.event}</span>
            </div>
          \`).join('');
        }

        modal.style.display = 'flex';
      }

      function closeAdminTransactionModal() {
        const modal = document.getElementById('adminTransactionDetailModal');
        if (modal) modal.style.display = 'none';
      }

      function exportAdminFinanceLedger() {
        const headers = ['TxID', 'GatewayRef', 'Timestamp', 'Gateway', 'Type', 'ConsultationId', 'Patient', 'PatientPhone', 'Doctor', 'DoctorBMDC', 'GrossAmount', 'PlatformFee', 'NetAmount', 'EscrowStatus'];
        const csvRows = [headers.join(',')];

        adminTransactionStore.forEach(tx => {
          const row = [
            tx.txId,
            tx.gatewayRef,
            \`"\${tx.timestamp}"\`,
            tx.gateway,
            \`"\${tx.typeLabel}"\`,
            tx.consultationId,
            \`"\${tx.patient.name}"\`,
            tx.patient.phone,
            \`"\${tx.doctor.name}"\`,
            tx.doctor.bmdc,
            tx.grossAmount,
            tx.platformFeeAmount,
            tx.netAmount,
            tx.escrowStatus
          ];
          csvRows.push(row.join(','));
        });

        const csvContent = 'data:text/csv;charset=utf-8,' + encodeURIComponent(csvRows.join('\\n'));
        const link = document.createElement('a');
        link.setAttribute('href', csvContent);
        link.setAttribute('download', \`HeloDoc_Master_Payment_Ledger_\${new Date().toISOString().slice(0, 10)}.csv\`);
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);

        if (typeof showAppToast === 'function') {
          showAppToast('Ledger CSV Exported 📊', 'Complete omnichannel payments and escrow audit report downloaded successfully.', 'success', '📊');
        }
      }

      function reconcileAdminMfsWebhooks() {
        if (typeof isAppOnline !== 'undefined' && !isAppOnline) {
          showOfflineRestrictedActionModal('MFS Webhook Reconciliation');
          return;
        }

        if (typeof showAppToast === 'function') {
          showAppToast('MFS Webhooks Reconciled ✓', 'All 1,864 transactions verified against bKash & Nagad settlement APIs. Zero discrepancies detected.', 'success', '⚡');
        }
      }

      window.adminTransactionStore = adminTransactionStore;
      window.renderAdminFinanceTable = renderAdminFinanceTable;
      window.filterAdminFinanceTable = filterAdminFinanceTable;
      window.setAdminFinanceTypeFilter = setAdminFinanceTypeFilter;
      window.openAdminTransactionModal = openAdminTransactionModal;
      window.closeAdminTransactionModal = closeAdminTransactionModal;
      window.exportAdminFinanceLedger = exportAdminFinanceLedger;
      window.reconcileAdminMfsWebhooks = reconcileAdminMfsWebhooks;
`;

const scriptCloseTag = '</script>\n\n</body>';
if (!html.includes('adminTransactionStore')) {
  html = html.replace(scriptCloseTag, financeJsCode + '\n    ' + scriptCloseTag);
  console.log("✓ Added finance JavaScript store and controllers");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully! New file size:", html.length);
