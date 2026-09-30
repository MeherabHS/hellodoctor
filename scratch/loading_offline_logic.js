/* ═════════════════════════════════════════════════════════════
   OFFLINE RESILIENCE & SKELETON LOADING CONTROLLER
   ═════════════════════════════════════════════════════════════ */
let isAppOnline = true;
let _onlineToastTimer = null;

function setNetworkOnlineState(isOnline, isUserInitiated = false) {
  isAppOnline = isOnline;
  document.body.classList.toggle('is-offline', !isOnline);

  // Update workbench toggle button UI
  const netBtn = document.getElementById('wbNetworkToggle');
  const netLabel = document.getElementById('wbNetworkLabel');
  if (netBtn && netLabel) {
    if (isOnline) {
      netBtn.style.background = '#10B981';
      netLabel.textContent = 'Online';
      netBtn.title = 'Current: Online. Click to simulate Offline Mode';
    } else {
      netBtn.style.background = '#D97706';
      netLabel.textContent = 'Offline (Cached)';
      netBtn.title = 'Current: Offline. Click to reconnect Online';
    }
  }

  if (!isOnline) {
    // Show Offline Status Modal on disconnect
    openOfflineStatusModal();
  } else {
    // Show Online Restored Toast
    closeOfflineStatusModal();
    closeOfflineActionGuardModal();
    showOnlineRecoveryToast();
  }
}

function toggleNetworkSimulation(forceState) {
  const nextState = (typeof forceState === 'boolean') ? forceState : !isAppOnline;
  setNetworkOnlineState(nextState, true);
}

function openOfflineStatusModal() {
  const modal = document.getElementById('offlineStatusModal');
  if (modal) modal.style.display = 'flex';
}

function closeOfflineStatusModal() {
  const modal = document.getElementById('offlineStatusModal');
  if (modal) modal.style.display = 'none';
}

function showOfflineRestrictedActionModal(actionName = 'This action') {
  const title = document.getElementById('offlineGuardActionTitle');
  if (title) {
    title.textContent = `${actionName} Unavailable Offline`;
  }
  const modal = document.getElementById('offlineActionGuardModal');
  if (modal) modal.style.display = 'flex';
}

function closeOfflineActionGuardModal() {
  const modal = document.getElementById('offlineActionGuardModal');
  if (modal) modal.style.display = 'none';
}

function showOnlineRecoveryToast() {
  const toast = document.getElementById('onlineRecoveryToast');
  if (!toast) return;
  if (_onlineToastTimer) clearTimeout(_onlineToastTimer);

  toast.classList.add('show');
  _onlineToastTimer = setTimeout(() => {
    toast.classList.remove('show');
  }, 4200);
}

/* Universal Loading Transition Simulation */
function simulateDataLoadingTransition(targetScreen = null) {
  const overlay = document.getElementById('globalLoadingOverlay');
  if (!overlay) return;

  const titleEl = document.getElementById('globalLoadingTitle');
  const subEl = document.getElementById('globalLoadingSubtitle');

  if (currentRole === 'admin') {
    if (titleEl) titleEl.textContent = 'Refreshing Central Telemetry & Ledgers...';
    if (subEl) subEl.textContent = 'Re-aggregating live WebRTC streams and bKash escrow disbursements';
  } else if (currentRole === 'doctor-web') {
    if (titleEl) titleEl.textContent = 'Synchronizing High-Volume Queue...';
    if (subEl) subEl.textContent = 'Checking DGDA prescription safety and incoming patient vitals';
  } else {
    if (titleEl) titleEl.textContent = 'Loading HelloDoctor Health Records...';
    if (subEl) subEl.textContent = 'Connecting to encrypted health enclaves & BMDC slot availability';
  }

  overlay.classList.add('active');

  setTimeout(() => {
    if (typeof targetScreen === 'number') {
      switchScreen(targetScreen);
    }
    overlay.classList.remove('active');
    showAppToast('Data Synchronized ✓', 'Latest healthcare records & telemetry successfully loaded.', 'success', '✓');
  }, 1600);
}

// Listen to browser network changes
window.addEventListener('online', () => setNetworkOnlineState(true));
window.addEventListener('offline', () => setNetworkOnlineState(false));
