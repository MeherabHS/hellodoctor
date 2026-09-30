const fs = require('fs');

const targetFile = 'prototype/index.html';
let html = fs.readFileSync(targetFile, 'utf8');

const targetStartStr = '<div style="margin:12px 18px 8px; background:#ECFDF5; border:1px solid #A7F3D0; border-radius:14px; padding:10px 14px; display:flex; align-items:center; gap:10px;">';
const targetStartIdx = html.indexOf(targetStartStr);

if (targetStartIdx !== -1) {
  const targetEndIdx = html.indexOf('</div>', html.indexOf('openDoctorReportViewer(\'thyroid\'', targetStartIdx)) + 6;
  console.log("Replacing chunk:");
  console.log(html.substring(targetStartIdx, targetEndIdx));

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

  html = html.substring(0, targetStartIdx) + newWaitingGalleryChunk + html.substring(targetEndIdx);
  fs.writeFileSync(targetFile, html, 'utf8');
  console.log("✓ Replaced waiting room card successfully!");
} else {
  console.log("targetStartStr not found");
}
