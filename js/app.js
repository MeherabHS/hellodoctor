/**
 * Application Controller & State Engine
 * Manages 15 screens, color palettes, discreet mode, and interactive clinical workflows.
 */

document.addEventListener('DOMContentLoaded', () => {
  // Global State
  const state = {
    currentScreen: 'scr-02', // Default to Home Dashboard
    isDiscreet: false,
    activePalette: 'warm-dignity',
    doseTakenToday: false,
    pinBuffer: [],
    selectedRefillOption: 'locker',
    adherenceStreak: AppData.patient.adherenceStreak
  };

  // DOM Elements
  const screenViews = document.querySelectorAll('.screen-view');
  const switcherPills = document.querySelectorAll('.pill-btn');
  const paletteBtns = document.querySelectorAll('.palette-btn');
  const discreetBtn = document.getElementById('discreetToggleBtn');
  const navItems = document.querySelectorAll('.nav-item');

  // ========================================================================
  // ROUTING & SCREEN NAVIGATION
  // ========================================================================
  function navigateTo(screenId) {
    state.currentScreen = screenId;

    // Update screen views
    screenViews.forEach(view => {
      if (view.id === screenId) {
        view.classList.add('active');
        view.scrollTop = 0;
      } else {
        view.classList.remove('active');
      }
    });

    // Update workbench pills
    switcherPills.forEach(pill => {
      if (pill.dataset.target === screenId) {
        pill.classList.add('active');
        pill.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
      } else {
        pill.classList.remove('active');
      }
    });

    // Update bottom nav active state if screen maps to a tab
    const tabMap = {
      'scr-02': 'tab-home',
      'scr-03': 'tab-home',
      'scr-04': 'tab-regimen',
      'scr-05': 'tab-regimen',
      'scr-07': 'tab-consult',
      'scr-08': 'tab-consult',
      'scr-09': 'tab-consult',
      'scr-10': 'tab-milestones',
      'scr-13': 'tab-lifeline',
      'scr-15': 'tab-profile'
    };

    const targetTab = tabMap[screenId];
    navItems.forEach(item => {
      if (item.id === targetTab) {
        item.classList.add('active');
      } else {
        item.classList.remove('active');
      }
    });

    // Re-render charts or dynamic elements if needed
    if (screenId === 'scr-10') {
      renderViralLoadChart();
    }
  }

  // Bind Switcher Pills
  switcherPills.forEach(pill => {
    pill.addEventListener('click', () => {
      const target = pill.dataset.target;
      if (target) navigateTo(target);
    });
  });

  // Bind Bottom Nav
  navItems.forEach(item => {
    item.addEventListener('click', (e) => {
      e.preventDefault();
      const targetScreen = item.dataset.screen;
      if (targetScreen) navigateTo(targetScreen);
    });
  });

  // Delegated in-screen links (e.g. data-navigate="scr-06")
  document.addEventListener('click', (e) => {
    const link = e.target.closest('[data-navigate]');
    if (link) {
      e.preventDefault();
      navigateTo(link.dataset.navigate);
    }
  });

  // ========================================================================
  // COLOR PALETTE SWITCHER
  // ========================================================================
  paletteBtns.forEach(btn => {
    btn.addEventListener('click', () => {
      const palette = btn.dataset.palette;
      state.activePalette = palette;
      document.body.setAttribute('data-palette', palette);
      paletteBtns.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
    });
  });

  // ========================================================================
  // DISCREET PRIVACY SHIELD TOGGLE
  // ========================================================================
  function toggleDiscreetMode() {
    state.isDiscreet = !state.isDiscreet;
    document.body.classList.toggle('discreet-mode', state.isDiscreet);
    discreetBtn.classList.toggle('active', state.isDiscreet);

    const dict = state.isDiscreet ? AppData.discreetDictionary.discreet : AppData.discreetDictionary.standard;

    // Update dynamic text nodes across all screens
    document.querySelectorAll('[data-bind-discreet]').forEach(el => {
      const key = el.dataset.bindDiscreet;
      if (dict[key]) {
        el.textContent = dict[key];
      }
    });

    // Update button text
    const labelSpan = discreetBtn.querySelector('.shield-label');
    if (labelSpan) {
      labelSpan.textContent = state.isDiscreet ? "Discreet Mode: ON" : "Privacy Shield: OFF";
    }
  }

  discreetBtn.addEventListener('click', toggleDiscreetMode);

  // ========================================================================
  // INTERACTIVE ADHERENCE LOGGING (1-Tap Mark Taken)
  // ========================================================================
  window.toggleDoseTaken = function() {
    state.doseTakenToday = !state.doseTakenToday;
    const cards = document.querySelectorAll('.pill-dose-card');
    const streakElements = document.querySelectorAll('.adherence-streak-number');

    cards.forEach(card => {
      card.classList.toggle('taken', state.doseTakenToday);
    });

    if (state.doseTakenToday) {
      state.adherenceStreak = AppData.patient.adherenceStreak + 1;
      showToast("🎉 Dose logged! Adherence streak: 49 days. Keep thriving!");
    } else {
      state.adherenceStreak = AppData.patient.adherenceStreak;
      showToast("Dose unlogged.");
    }

    streakElements.forEach(el => {
      el.textContent = state.adherenceStreak;
    });
  };

  // ========================================================================
  // SCREEN 1: PIN UNLOCK SIMULATOR
  // ========================================================================
  window.pressPinKey = function(key) {
    if (key === 'delete') {
      state.pinBuffer.pop();
    } else if (state.pinBuffer.length < 4) {
      state.pinBuffer.push(key);
    }

    // Update dots
    const dots = document.querySelectorAll('.pin-dot');
    dots.forEach((dot, idx) => {
      dot.classList.toggle('filled', idx < state.pinBuffer.length);
    });

    // When 4 digits entered, auto-unlock
    if (state.pinBuffer.length === 4) {
      setTimeout(() => {
        state.pinBuffer = [];
        dots.forEach(d => d.classList.remove('filled'));
        navigateTo('scr-02'); // Navigate to Dashboard
        showToast("✓ Authenticated securely");
      }, 250);
    }
  };

  // ========================================================================
  // SCREEN 6: MISSED DOSE CALCULATOR
  // ========================================================================
  window.selectMissedHours = function(hours, btn) {
    document.querySelectorAll('.time-chip').forEach(c => c.classList.remove('active'));
    btn.classList.add('active');

    const resultBox = document.getElementById('missedResultBox');
    if (hours <= 12) {
      resultBox.innerHTML = `
        <div style="background: var(--color-success-surface); border: 1.5px solid var(--color-success); border-radius: 14px; padding: 16px; margin-top: 14px;">
          <div style="font-weight: 800; color: var(--color-success); font-size: 15px; margin-bottom: 4px;">
            ✓ Safe Window: Take 1 Dose Now
          </div>
          <div style="font-size: 13px; color: var(--color-text-main); line-height: 19px;">
            It has been <strong>${hours} hours</strong> since your normal time. Because TLD has a long protective half-life, take your tablet right now with water, then resume your regular dose tomorrow night as usual.
          </div>
        </div>
      `;
    } else {
      resultBox.innerHTML = `
        <div style="background: var(--color-gold-surface); border: 1.5px solid var(--color-gold); border-radius: 14px; padding: 16px; margin-top: 14px;">
          <div style="font-weight: 800; color: var(--color-gold); font-size: 15px; margin-bottom: 4px;">
            ⚠️ Greater than 12 Hours: Skip & Resume
          </div>
          <div style="font-size: 13px; color: var(--color-text-main); line-height: 19px;">
            It has been <strong>${hours} hours</strong>. Do NOT take a double dose. Skip this missed pill and take your next single dose at your usual scheduled time tonight.
          </div>
        </div>
      `;
    }
  };

  // ========================================================================
  // SCREEN 10: VIRAL LOAD CHART GENERATOR (SVG)
  // ========================================================================
  function renderViralLoadChart() {
    const container = document.getElementById('viralLoadSvgContainer');
    if (!container) return;

    const data = AppData.viralLoadHistory;
    const width = 310;
    const height = 150;
    const padding = { top: 20, right: 20, bottom: 30, left: 35 };

    // Logarithmic scale representation for Viral load (45,000 down to 10)
    const points = [
      { x: 35, y: 25, val: "45k", date: "2021" },
      { x: 80, y: 55, val: "3.8k", date: "'22" },
      { x: 130, y: 85, val: "420", date: "'22" },
      { x: 180, y: 110, val: "75", date: "'23" },
      { x: 230, y: 128, val: "<20", date: "'23" },
      { x: 280, y: 130, val: "<20", date: "Now" }
    ];

    let pathD = `M ${points[0].x} ${points[0].y}`;
    for (let i = 1; i < points.length; i++) {
      const prev = points[i - 1];
      const curr = points[i];
      const cpX = (prev.x + curr.x) / 2;
      pathD += ` C ${cpX} ${prev.y}, ${cpX} ${curr.y}, ${curr.x} ${curr.y}`;
    }

    // Fill area path
    const fillD = `${pathD} L ${points[points.length - 1].x} ${height - padding.bottom} L ${points[0].x} ${height - padding.bottom} Z`;

    container.innerHTML = `
      <svg width="100%" height="100%" viewBox="0 0 ${width} ${height}">
        <defs>
          <linearGradient id="chartGrad" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stop-color="var(--color-primary-light)" stop-opacity="0.35"/>
            <stop offset="100%" stop-color="var(--color-primary-light)" stop-opacity="0.0"/>
          </linearGradient>
        </defs>

        <!-- Undetectable Safety Zone (<50 copies/mL) -->
        <rect x="35" y="115" width="250" height="25" fill="var(--color-success-surface)" opacity="0.6" rx="4"/>
        <text x="40" y="127" font-size="9" fill="var(--color-success)" font-weight="700">TARGET: UNDETECTABLE (&lt;50 copies/mL)</text>

        <!-- Area Fill -->
        <path d="${fillD}" fill="url(#chartGrad)"/>

        <!-- Line Curve -->
        <path d="${pathD}" fill="none" stroke="var(--color-primary)" stroke-width="3" stroke-linecap="round"/>

        <!-- Data Points -->
        ${points.map((p, idx) => `
          <circle cx="${p.x}" cy="${p.y}" r="${idx === points.length - 1 ? 6 : 4}" fill="${idx === points.length - 1 ? 'var(--color-primary-light)' : 'var(--color-primary)'}" stroke="#FFFFFF" stroke-width="2"/>
          <text x="${p.x}" y="${p.y - 8}" font-size="9" font-weight="700" fill="var(--color-text-main)" text-anchor="middle">${p.val}</text>
          <text x="${p.x}" y="${height - 12}" font-size="9" font-weight="600" fill="var(--color-text-muted)" text-anchor="middle">${p.date}</text>
        `).join('')}
      </svg>
    `;
  }

  // ========================================================================
  // SCREEN 11: REFILL METHOD SELECTOR
  // ========================================================================
  window.selectRefillMethod = function(method, card) {
    state.selectedRefillOption = method;
    document.querySelectorAll('.option-select-card').forEach(c => c.classList.remove('selected'));
    card.classList.add('selected');
  };

  // ========================================================================
  // TOAST NOTIFICATION UTILITY
  // ========================================================================
  function showToast(message) {
    let toast = document.getElementById('appToast');
    if (!toast) {
      toast = document.createElement('div');
      toast.id = 'appToast';
      toast.style.cssText = `
        position: absolute;
        bottom: 84px;
        left: 20px;
        right: 20px;
        background: #182220;
        color: #FFFFFF;
        padding: 12px 18px;
        border-radius: 16px;
        font-size: 13px;
        font-weight: 600;
        text-align: center;
        box-shadow: 0 10px 30px rgba(0,0,0,0.3);
        z-index: 1000;
        opacity: 0;
        transform: translateY(10px);
        transition: all 250ms cubic-bezier(0.175, 0.885, 0.32, 1.275);
        pointer-events: none;
      `;
      document.querySelector('.phone-screen').appendChild(toast);
    }
    toast.textContent = message;
    toast.style.opacity = '1';
    toast.style.transform = 'translateY(0)';

    setTimeout(() => {
      toast.style.opacity = '0';
      toast.style.transform = 'translateY(10px)';
    }, 2800);
  }

  // Initialize
  navigateTo('scr-02');
});
