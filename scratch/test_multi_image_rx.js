// Prototype test for multi-image prescription upload & viewer
const fs = require('fs');

console.log("Testing multi-image prescription upload and gallery rendering...");

let patientPrescriptionImages = [
  {
    id: 'rx-page-1',
    name: 'Prescription_Page_1.jpg',
    size: '1.2 MB',
    type: 'image/jpeg',
    url: 'data:image/jpeg;base64,/9j/4AAQSkZJRg...',
    isImage: true,
    isPreset: false
  },
  {
    id: 'rx-page-2',
    name: 'Prescription_Page_2.jpg',
    size: '1.4 MB',
    type: 'image/jpeg',
    url: 'data:image/jpeg;base64,/9j/4AAQSkZJRg...',
    isImage: true,
    isPreset: false
  },
  {
    id: 'rx-page-3',
    name: 'Lab_Report_Slip_Page_3.jpg',
    size: '0.9 MB',
    type: 'image/jpeg',
    url: 'data:image/jpeg;base64,/9j/4AAQSkZJRg...',
    isImage: true,
    isPreset: false
  }
];

console.log("Current total images:", patientPrescriptionImages.length);
console.log("Max limit:", 5);
console.log("Available slots:", 5 - patientPrescriptionImages.length);

function getMultiPagePaginationText(currentIndex) {
  return `Prescription Page ${currentIndex + 1} of ${patientPrescriptionImages.length}`;
}

console.log("Page 0 label:", getMultiPagePaginationText(0));
console.log("Page 1 label:", getMultiPagePaginationText(1));
console.log("Page 2 label:", getMultiPagePaginationText(2));
