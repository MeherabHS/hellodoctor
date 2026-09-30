const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

console.log("Original file size:", html.length);

// 1. Update file input to support multiple files
const oldFileInput = '<input type="file" id="patientPreConsultFileInput" accept="image/*,application/pdf" style="display:none;" onchange="handlePatientFileSelection(event)">';
const newFileInput = '<input type="file" id="patientPreConsultFileInput" accept="image/*,application/pdf" multiple style="display:none;" onchange="handlePatientFileSelection(event)">';

if (html.includes(oldFileInput)) {
  html = html.replace(oldFileInput, newFileInput);
  console.log("✓ Updated file input to allow multiple selection");
}

// 2. Update dropzone text in intake modal
const oldDropzoneText = `<div style="font-size:13px; font-weight:750; color:var(--text-main); margin-top:8px;">Upload Prescription or Test Image</div>
              <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Tap to capture with camera or browse device files</div>
              <div style="display:inline-flex; gap:6px; margin-top:8px;">
                <span class="badge badge-blue" style="font-size:9.5px;">📸 Camera</span>
                <span class="badge badge-green" style="font-size:9.5px;">📁 PDF / Images</span>
              </div>`;

const newDropzoneText = `<div style="font-size:13px; font-weight:750; color:var(--text-main); margin-top:8px;">Upload Prescription or Test Images (Up to 5 Max)</div>
              <div style="font-size:11px; color:var(--text-sub); margin-top:2px;">Capture multiple prescription pages via camera or file gallery</div>
              <div style="display:inline-flex; gap:6px; margin-top:8px;">
                <span class="badge badge-blue" style="font-size:9.5px;">📸 Camera Multi-Page</span>
                <span class="badge badge-green" style="font-size:9.5px;">📁 1 to 5 Photos Max</span>
              </div>`;

if (html.includes(oldDropzoneText)) {
  html = html.replace(oldDropzoneText, newDropzoneText);
  console.log("✓ Updated dropzone copy for multi-image upload up to 5 max");
}

// 3. Update the intake attached doc preview into a multi-page gallery container
const oldAttachedCardStart = '<!-- Live Attached Document Preview Card -->\n            <div class="attached-doc-card" id="patientAttachedPreviewCard" style="display:flex;">';
const oldAttachedCardEnd = '<button class="attached-doc-remove" onclick="removeAttachedReport(event)" title="Change / Remove File">✕</button>\n            </div>';

const intakeGalleryChunk = `<!-- Multi-Image Prescription Gallery Container (Up to 5 Pages Max) -->
            <div id="patientPrescriptionGalleryContainer" style="margin-top:14px;">
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
                <span style="font-size:11px; font-weight:800; color:var(--text-sub); text-transform:uppercase; letter-spacing:0.3px;">Attached Prescription Pages</span>
                <span class="badge badge-green" id="rxGalleryCountBadge" style="font-size:9.5px; font-weight:800;">1 of 5 Photos</span>
              </div>

              <!-- Dynamic Multi-Page Prescription List -->
              <div id="patientPrescriptionList" style="display:flex; flex-direction:column; gap:8px;">
                <!-- Populated via renderPatientPrescriptionGallery() -->
              </div>

              <!-- Button to Add Another Page (if < 5) -->
              <button type="button" id="btnAddMorePrescriptionPhoto" onclick="document.getElementById('patientPreConsultFileInput').click()" style="margin-top:8px; width:100%; padding:9px 12px; border:1.5px dashed #10B981; background:#F0FDF4; border-radius:10px; font-size:11.5px; font-weight:750; color:#065F46; display:flex; align-items:center; justify-content:center; gap:6px; cursor:pointer;">
                <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                <span>+ Add Another Prescription Image (Max 5)</span>
              </button>

              <!-- Legacy hidden fields for backward compatibility -->
              <div class="attached-doc-card" id="patientAttachedPreviewCard" style="display:none;">
                <div class="attached-doc-thumb" id="patientAttachedThumb"></div>
                <div class="attached-doc-name" id="patientAttachedName">Thyroid_Function_Profile_22Sep.pdf</div>
                <div class="attached-doc-meta" id="patientAttachedMeta">1.8 MB • Ready</div>
              </div>
            </div>`;

const cardIdx1 = html.indexOf('<!-- Live Attached Document Preview Card -->');
if (cardIdx1 !== -1) {
  const cardIdx2 = html.indexOf('<!-- Chief Complaint Notes -->', cardIdx1);
  if (cardIdx2 !== -1) {
    html = html.substring(0, cardIdx1) + intakeGalleryChunk + '\n\n            ' + html.substring(cardIdx2);
    console.log("✓ Replaced single intake attachment card with multi-page prescription gallery container");
  }
}

// 4. Update Scheduled Consultation screen (#view-10) with multi-image gallery
const oldWaitingCardStart = '<div style="margin:12px 18px 8px; background:#ECFDF5; border:1px solid #A7F3D0; border-radius:14px; padding:10px 14px; display:flex; align-items:center; gap:10px;">';
const oldWaitingCardEnd = '<button class="slot-subtle-btn" style="padding:4px 8px; font-size:10px; background:#fff; color:#065F46; border:1px solid #A7F3D0; border-radius:8px;" onclick="openDoctorReportViewer(\'thyroid\', \'Nusrat Jahan\')">View</button>\n            </div>';

const newWaitingGalleryChunk = `<!-- Multi-Image Prescription Gallery Card in Scheduled Consultation (Max 5) -->
            <div style="margin:12px 18px 8px; background:#ECFDF5; border:1px solid #A7F3D0; border-radius:14px; padding:12px 14px;" id="waitingRoomPrescriptionCard">
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
                <div style="display:flex; align-items:center; gap:8px;">
                  <span style="font-size:18px;">📄</span>
                  <div>
                    <div style="font-size:12px; font-weight:800; color:#065F46;">Attached Prescriptions &amp; Records</div>
                    <div style="font-size:10px; color:#047857;" id="waitingRoomReportSub">Attached for Dr. Sabrina • Ready for review</div>
                  </div>
                </div>
                <div style="display:flex; align-items:center; gap:6px;">
                  <span class="badge badge-green" id="wrPrescriptionCountBadge" style="font-size:9.5px; font-weight:800;">1 of 5 Pages</span>
                  <button class="slot-subtle-btn" style="padding:4px 10px; font-size:10.5px; font-weight:750; background:#fff; color:#065F46; border:1px solid #A7F3D0; border-radius:8px; cursor:pointer;" onclick="openMultiPagePrescriptionViewer(0)">View All</button>
                </div>
              </div>

              <!-- Horizontal Thumbnails Strip (Up to 5 images) -->
              <div id="wrPrescriptionThumbnailsStrip" style="display:flex; gap:8px; overflow-x:auto; padding:4px 2px; margin-top:6px;">
                <!-- Populated via updateWaitingRoomPrescriptionGallery() -->
              </div>

              <!-- Preserved legacy element for backwards compatibility -->
              <span id="waitingRoomReportName" style="display:none;">Thyroid_Function_Profile_22Sep.pdf</span>
            </div>`;

if (html.includes(oldWaitingCardStart)) {
  const wStart = html.indexOf(oldWaitingCardStart);
  const wEnd = html.indexOf('</div>', html.indexOf('openDoctorReportViewer(\'thyroid\'', wStart));
  if (wEnd !== -1) {
    html = html.substring(0, wStart) + newWaitingGalleryChunk + html.substring(wEnd + 6);
    console.log("✓ Replaced waiting room single report bar with multi-image prescription gallery card");
  }
}

// 5. In #labReportModal, add the multi-page pagination bar
const oldPhotoContainer = '<div id="pdfUploadedPhotoContainer" style="display:none; text-align:center; margin-bottom:10px;">';
const newPhotoContainer = `<!-- Multi-Page Pagination Bar (when viewing attached multi-image prescription) -->
              <div id="pdfMultiPageBar" style="display:none; justify-content:space-between; align-items:center; background:#F0FDF4; border:1px solid #A7F3D0; border-radius:10px; padding:7px 12px; margin-bottom:10px;">
                <button type="button" onclick="switchPrescriptionViewerPage(-1)" id="btnPrevRxPage" style="border:1px solid #A7F3D0; background:#fff; padding:4px 10px; border-radius:6px; font-weight:800; font-size:11px; color:#065F46; cursor:pointer; display:inline-flex; align-items:center; gap:4px;">
                  <span>‹ Prev Page</span>
                </button>

                <div style="text-align:center;">
                  <span style="font-size:12px; font-weight:850; color:#065F46;" id="pdfMultiPageDisplay">Prescription Page 1 of 1</span>
                  <div style="font-size:9.5px; color:#047857;" id="pdfMultiPageFileName">Thyroid_Function_Profile_22Sep.pdf</div>
                </div>

                <button type="button" onclick="switchPrescriptionViewerPage(1)" id="btnNextRxPage" style="border:1px solid #A7F3D0; background:#fff; padding:4px 10px; border-radius:6px; font-weight:800; font-size:11px; color:#065F46; cursor:pointer; display:inline-flex; align-items:center; gap:4px;">
                  <span>Next Page ›</span>
                </button>
              </div>

              <!-- Multi-Page Thumbnails Strip inside Viewer -->
              <div id="pdfMultiPageThumbsStrip" style="display:none; justify-content:center; gap:6px; margin-bottom:10px; flex-wrap:wrap;"></div>

              <div id="pdfUploadedPhotoContainer" style="display:none; text-align:center; margin-bottom:10px;">`;

if (html.includes(oldPhotoContainer) && !html.includes('id="pdfMultiPageBar"')) {
  html = html.replace(oldPhotoContainer, newPhotoContainer);
  console.log("✓ Added multi-page pagination bar and thumb strip to lab report viewer modal");
}

// 6. Update JavaScript handlers for multi-page prescription upload & viewer
const oldHandleFn = 'function handlePatientFileSelection(event) {';
const oldSelectPresetFn = 'function selectPresetReport(presetKey) {';

const newMultiUploadJs = `
// ═════════════════════════════════════════════════════════════
// MULTI-IMAGE PRESCRIPTION UPLOAD & GALLERY ENGINE (UP TO 5 MAX)
// ═════════════════════════════════════════════════════════════
const MAX_PRESCRIPTION_IMAGES = 5;
let currentActivePrescriptionPageIndex = 0;

let patientPrescriptionImages = [
  {
    id: 'rx-doc-preset-1',
    name: 'Thyroid_Function_Profile_22Sep.pdf',
    size: '1.8 MB',
    type: 'application/pdf',
    url: null,
    isImage: false,
    isPreset: true,
    presetType: 'thyroid'
  }
];

function handlePatientFileSelection(event) {
  const files = Array.from(event.target.files || []);
  if (!files || files.length === 0) return;

  // If currently only holding the default preset, replace it with actual patient upload
  if (patientPrescriptionImages.length === 1 && patientPrescriptionImages[0].isPreset) {
    patientPrescriptionImages = [];
  }

  const currentCount = patientPrescriptionImages.length;
  if (currentCount >= MAX_PRESCRIPTION_IMAGES) {
    if (typeof showAppToast === 'function') {
      showAppToast('Limit Reached ⚠️', 'Maximum 5 prescription images allowed. Remove a page to add another.', 'info', '⚠️');
    } else {
      alert('Maximum 5 prescription images allowed.');
    }
    event.target.value = '';
    return;
  }

  const remainingSlots = MAX_PRESCRIPTION_IMAGES - currentCount;
  const filesToAdd = files.slice(0, remainingSlots);

  if (files.length > remainingSlots) {
    if (typeof showAppToast === 'function') {
      showAppToast('Capped at 5 Images ℹ️', \`Maximum 5 images allowed. First \${remainingSlots} selected photos were attached.\`, 'info', '📷');
    }
  }

  let processedCount = 0;
  filesToAdd.forEach((file, fIdx) => {
    const isImage = file.type.startsWith('image/');
    const sizeMb = (file.size / (1024 * 1024)).toFixed(1);
    const sizeFormatted = sizeMb > 0 ? sizeMb + ' MB' : '0.8 MB';

    if (isImage) {
      const reader = new FileReader();
      reader.onload = (e) => {
        patientPrescriptionImages.push({
          id: 'rx-img-' + Date.now() + '-' + Math.floor(Math.random() * 1000),
          name: file.name,
          size: sizeFormatted,
          type: file.type,
          url: e.target.result,
          isImage: true,
          isPreset: false
        });
        processedCount++;
        if (processedCount === filesToAdd.length) {
          onPrescriptionImagesUpdated();
        }
      };
      reader.readAsDataURL(file);
    } else {
      patientPrescriptionImages.push({
        id: 'rx-doc-' + Date.now() + '-' + Math.floor(Math.random() * 1000),
        name: file.name,
        size: sizeFormatted,
        type: file.type,
        url: null,
        isImage: false,
        isPreset: false
      });
      processedCount++;
      if (processedCount === filesToAdd.length) {
        onPrescriptionImagesUpdated();
      }
    }
  });

  event.target.value = '';
}

function onPrescriptionImagesUpdated() {
  currentSelectedReportName = patientPrescriptionImages[0]?.name || 'Prescription.pdf';
  renderPatientPrescriptionGallery();
  updateWaitingRoomPrescriptionGallery();

  const total = patientPrescriptionImages.length;
  if (typeof showAppToast === 'function') {
    showAppToast(
      'Prescription Attached 📄',
      \`\${total} of \${MAX_PRESCRIPTION_IMAGES} pages attached for Dr. Sabrina's review.\`,
      'success',
      '📎'
    );
  }
}

function removePatientPrescriptionImage(index, event) {
  if (event) event.stopPropagation();

  if (index >= 0 && index < patientPrescriptionImages.length) {
    const removed = patientPrescriptionImages.splice(index, 1);
    
    // If all removed, restore the default preset so doctor always has baseline record
    if (patientPrescriptionImages.length === 0) {
      patientPrescriptionImages.push({
        id: 'rx-doc-preset-1',
        name: 'Thyroid_Function_Profile_22Sep.pdf',
        size: '1.8 MB',
        type: 'application/pdf',
        url: null,
        isImage: false,
        isPreset: true,
        presetType: 'thyroid'
      });
    }

    currentSelectedReportName = patientPrescriptionImages[0]?.name || 'Prescription.pdf';
    renderPatientPrescriptionGallery();
    updateWaitingRoomPrescriptionGallery();

    if (typeof showAppToast === 'function') {
      showAppToast(
        'Page Removed ✕',
        \`\${patientPrescriptionImages.length} prescription \${patientPrescriptionImages.length === 1 ? 'page' : 'pages'} remaining.\`,
        'info',
        '🗑️'
      );
    }
  }
}

function renderPatientPrescriptionGallery() {
  const listEl = document.getElementById('patientPrescriptionList');
  const countBadge = document.getElementById('rxGalleryCountBadge');
  const btnAddMore = document.getElementById('btnAddMorePrescriptionPhoto');

  const total = patientPrescriptionImages.length;
  if (countBadge) {
    countBadge.textContent = \`\${total} of \${MAX_PRESCRIPTION_IMAGES} Photos\`;
    countBadge.className = total === MAX_PRESCRIPTION_IMAGES ? 'badge badge-amber' : 'badge badge-green';
  }

  if (btnAddMore) {
    btnAddMore.style.display = total >= MAX_PRESCRIPTION_IMAGES ? 'none' : 'flex';
    const span = btnAddMore.querySelector('span');
    if (span) span.textContent = \`+ Add Another Prescription Image (\${total + 1} of \${MAX_PRESCRIPTION_IMAGES})\`;
  }

  if (listEl) {
    listEl.innerHTML = patientPrescriptionImages.map((img, idx) => {
      const isPreset = img.isPreset;
      const thumbContent = img.isImage && img.url
        ? \`<img src="\${img.url}" style="width:100%; height:100%; object-fit:cover; border-radius:6px;" alt="Page \${idx + 1}">\`
        : \`<span style="font-size:18px;">📄</span>\`;

      return \`
        <div class="attached-doc-card" style="display:flex; align-items:center; gap:10px; padding:10px 12px; background:#F0FDF4; border:1px solid #A7F3D0; border-radius:12px;">
          <div class="attached-doc-thumb" style="width:38px; height:38px; border-radius:8px; background:#FFF; border:1px solid #A7F3D0; display:grid; place-items:center; overflow:hidden; flex-shrink:0;">
            \${thumbContent}
          </div>
          <div style="flex:1; min-width:0;">
            <div style="display:flex; align-items:center; gap:6px;">
              <span class="badge badge-green" style="font-size:8.5px; padding:1px 5px; font-weight:800;">Page \${idx + 1}</span>
              <div style="font-weight:750; font-size:11.5px; color:#065F46; white-space:nowrap; overflow:hidden; text-overflow:ellipsis;">
                \${img.name}
              </div>
            </div>
            <div style="font-size:10px; color:#047857; margin-top:2px;">
              \${img.size} • \${img.isImage ? 'Prescription Photo' : 'Clinical Report'} • Ready for Doctor
            </div>
          </div>
          <button type="button" onclick="removePatientPrescriptionImage(\${idx}, event)" style="border:none; background:#FEE2E2; color:#DC2626; width:26px; height:26px; border-radius:50%; font-size:11px; cursor:pointer; display:grid; place-items:center; flex-shrink:0;" title="Remove this prescription image">✕</button>
        </div>
      \`;
    }).join('');
  }

  // Also sync legacy single-file elements for backwards compatibility
  const legacyName = document.getElementById('patientAttachedName');
  const legacyMeta = document.getElementById('patientAttachedMeta');
  if (legacyName && patientPrescriptionImages[0]) legacyName.textContent = patientPrescriptionImages[0].name;
  if (legacyMeta && patientPrescriptionImages[0]) legacyMeta.textContent = \`\${patientPrescriptionImages[0].size} • Ready\`;
}

function updateWaitingRoomPrescriptionGallery() {
  const countBadge = document.getElementById('wrPrescriptionCountBadge');
  const subEl = document.getElementById('waitingRoomReportSub');
  const stripEl = document.getElementById('wrPrescriptionThumbnailsStrip');
  const legacyName = document.getElementById('waitingRoomReportName');

  const total = patientPrescriptionImages.length;
  if (countBadge) {
    countBadge.textContent = \`\${total} of \${MAX_PRESCRIPTION_IMAGES} Pages\`;
    countBadge.className = total === MAX_PRESCRIPTION_IMAGES ? 'badge badge-amber' : 'badge badge-green';
  }

  if (subEl) {
    subEl.textContent = \`\${total} \${total === 1 ? 'prescription document' : 'prescription images'} attached • Ready for Dr. Sabrina\`;
  }

  if (legacyName && patientPrescriptionImages[0]) {
    legacyName.textContent = patientPrescriptionImages[0].name;
  }

  if (stripEl) {
    let htmlStr = patientPrescriptionImages.map((img, idx) => {
      const thumbContent = img.isImage && img.url
        ? \`<img src="\${img.url}" style="width:100%; height:100%; object-fit:cover;" alt="Page \${idx + 1}">\`
        : \`<div style="display:grid; place-items:center; width:100%; height:100%; font-size:18px; color:#059669; background:#ECFDF5;">📄</div>\`;

      return \`
        <div onclick="openMultiPagePrescriptionViewer(\${idx})" style="display:flex; flex-direction:column; align-items:center; gap:3px; cursor:pointer; flex-shrink:0;" title="Click to view Page \${idx + 1}">
          <div style="width:48px; height:58px; border-radius:8px; border:1.5px solid #10B981; overflow:hidden; background:#fff; box-shadow:0 1px 3px rgba(0,0,0,0.08);">
            \${thumbContent}
          </div>
          <span style="font-size:9.5px; font-weight:800; color:#065F46;">Page \${idx + 1}</span>
        </div>
      \`;
    }).join('');

    // If less than 5, render "+ Add Page" chip directly in waiting room
    if (total < MAX_PRESCRIPTION_IMAGES) {
      htmlStr += \`
        <div onclick="document.getElementById('patientPreConsultFileInput').click()" style="display:flex; flex-direction:column; align-items:center; gap:3px; cursor:pointer; flex-shrink:0;" title="Add another prescription image (up to 5 max)">
          <div style="width:48px; height:58px; border-radius:8px; border:1.5px dashed #10B981; background:#F0FDF4; display:grid; place-items:center; color:#059669;">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          </div>
          <span style="font-size:9.5px; font-weight:700; color:#047857;">+ Add</span>
        </div>
      \`;
    }

    stripEl.innerHTML = htmlStr;
  }
}

function openMultiPagePrescriptionViewer(pageIndex = 0) {
  if (patientPrescriptionImages.length === 0) return;
  currentActivePrescriptionPageIndex = Math.max(0, Math.min(pageIndex, patientPrescriptionImages.length - 1));

  const imgObj = patientPrescriptionImages[currentActivePrescriptionPageIndex];

  // Open the lab report / prescription modal
  const modal = document.getElementById('labReportModal');
  if (modal) modal.style.display = 'flex';

  const multiBar = document.getElementById('pdfMultiPageBar');
  const thumbsStrip = document.getElementById('pdfMultiPageThumbsStrip');
  const pageDisp = document.getElementById('pdfMultiPageDisplay');
  const fileDisp = document.getElementById('pdfMultiPageFileName');
  const prevBtn = document.getElementById('btnPrevRxPage');
  const nextBtn = document.getElementById('btnNextRxPage');
  const photoContainer = document.getElementById('pdfUploadedPhotoContainer');
  const photoImg = document.getElementById('pdfUploadedPhotoImg');
  const pdfArea = document.getElementById('pdfDocumentViewArea');

  // If there are multiple images, show pagination bar
  if (multiBar) {
    multiBar.style.display = 'flex';
    if (pageDisp) pageDisp.textContent = \`Prescription Page \${currentActivePrescriptionPageIndex + 1} of \${patientPrescriptionImages.length}\`;
    if (fileDisp) fileDisp.textContent = imgObj.name;
    if (prevBtn) {
      prevBtn.disabled = currentActivePrescriptionPageIndex === 0;
      prevBtn.style.opacity = currentActivePrescriptionPageIndex === 0 ? '0.4' : '1';
    }
    if (nextBtn) {
      nextBtn.disabled = currentActivePrescriptionPageIndex >= patientPrescriptionImages.length - 1;
      nextBtn.style.opacity = currentActivePrescriptionPageIndex >= patientPrescriptionImages.length - 1 ? '0.4' : '1';
    }
  }

  // Thumbnails bar inside viewer
  if (thumbsStrip) {
    thumbsStrip.style.display = patientPrescriptionImages.length > 1 ? 'flex' : 'none';
    thumbsStrip.innerHTML = patientPrescriptionImages.map((p, pIdx) => {
      const isActive = pIdx === currentActivePrescriptionPageIndex;
      return \`
        <button type="button" onclick="openMultiPagePrescriptionViewer(\${pIdx})" style="padding:3px 8px; border-radius:6px; font-size:10px; font-weight:800; border:1px solid \${isActive ? '#059669' : '#CBD5E1'}; background:\${isActive ? '#ECFDF5' : '#FFF'}; color:\${isActive ? '#065F46' : '#64748B'}; cursor:pointer;">
          Page \${pIdx + 1}
        </button>
      \`;
    }).join('');
  }

  // If the active page is an image, display it prominently
  if (imgObj.isImage && imgObj.url) {
    if (photoContainer) photoContainer.style.display = 'block';
    if (photoImg) photoImg.src = imgObj.url;
    if (pdfArea) {
      // Hide or deemphasize the default lab table when viewing actual prescription photos
      const tableMini = pdfArea.querySelector('.pdf-table-mini');
      if (tableMini) tableMini.style.display = 'none';
    }
  } else {
    // If it's the preset PDF or report
    if (photoContainer) photoContainer.style.display = 'none';
    if (pdfArea) {
      const tableMini = pdfArea.querySelector('.pdf-table-mini');
      if (tableMini) tableMini.style.display = 'table';
    }
  }
}

function switchPrescriptionViewerPage(delta) {
  const newIndex = currentActivePrescriptionPageIndex + delta;
  if (newIndex >= 0 && newIndex < patientPrescriptionImages.length) {
    openMultiPagePrescriptionViewer(newIndex);
  }
}
`;

// Replace handlePatientFileSelection and related helpers in prototype/index.html
const fnStart = html.indexOf('function handlePatientFileSelection(event) {');
const fnEnd = html.indexOf('function selectPresetReport(presetKey) {');

if (fnStart !== -1 && fnEnd !== -1) {
  html = html.substring(0, fnStart) + newMultiUploadJs + '\n\n' + html.substring(fnEnd);
  console.log("✓ Replaced handlePatientFileSelection with full Multi-Image Prescription Engine");
}

// Ensure initial gallery rendering runs on startup
const initHook = `
      // Initialize multi-page prescription gallery
      try {
        renderPatientPrescriptionGallery();
        updateWaitingRoomPrescriptionGallery();
      } catch (e) {
        console.warn('Prescription gallery initial render deferred:', e);
      }
`;

if (!html.includes('renderPatientPrescriptionGallery()')) {
  html = html.replace('// Initial render of grievances table and badge', initHook + '\n      // Initial render of grievances table and badge');
  console.log("✓ Hooked initial render for prescription galleries");
}

// Export window globals
const windowExports = `
      window.patientPrescriptionImages = patientPrescriptionImages;
      window.handlePatientFileSelection = handlePatientFileSelection;
      window.removePatientPrescriptionImage = removePatientPrescriptionImage;
      window.renderPatientPrescriptionGallery = renderPatientPrescriptionGallery;
      window.updateWaitingRoomPrescriptionGallery = updateWaitingRoomPrescriptionGallery;
      window.openMultiPagePrescriptionViewer = openMultiPagePrescriptionViewer;
      window.switchPrescriptionViewerPage = switchPrescriptionViewerPage;
`;

if (!html.includes('window.patientPrescriptionImages = patientPrescriptionImages;')) {
  html = html.replace('window.updatePatientGrievanceTelemetryUI = updatePatientGrievanceTelemetryUI;', 'window.updatePatientGrievanceTelemetryUI = updatePatientGrievanceTelemetryUI;\n' + windowExports);
  console.log("✓ Added window globals for multi-prescription upload");
}

fs.writeFileSync(targetFile, html, 'utf8');
console.log("File saved successfully! New file size:", html.length);
