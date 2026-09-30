// Test multi-image prescription logic
const MAX_PRESCRIPTION_IMAGES = 5;

let patientUploadedPrescriptions = [
  {
    id: 'rx-1',
    name: 'Thyroid_Function_Profile_22Sep.pdf',
    size: '1.8 MB',
    type: 'application/pdf',
    url: null,
    isImage: false,
    isPreset: true
  }
];

function addFilesSimulated(files) {
  const currentCount = patientUploadedPrescriptions.length;
  if (currentCount >= MAX_PRESCRIPTION_IMAGES) {
    return { success: false, message: 'Max 5 reached' };
  }

  const allowed = files.slice(0, MAX_PRESCRIPTION_IMAGES - currentCount);
  allowed.forEach((f, idx) => {
    patientUploadedPrescriptions.push({
      id: 'rx-' + (patientUploadedPrescriptions.length + 1),
      name: f.name,
      size: f.size,
      type: f.type,
      url: 'data:image/jpeg;base64,sample...',
      isImage: true,
      isPreset: false
    });
  });

  return { success: true, count: patientUploadedPrescriptions.length };
}

console.log("Initial count:", patientUploadedPrescriptions.length);
console.log("Adding 3 images:", addFilesSimulated([
  { name: 'Rx_Page_1.jpg', size: '1.2 MB', type: 'image/jpeg' },
  { name: 'Rx_Page_2.jpg', size: '1.4 MB', type: 'image/jpeg' },
  { name: 'Rx_Page_3.jpg', size: '0.9 MB', type: 'image/jpeg' }
]));

console.log("Adding 3 more images (should cap at 5):", addFilesSimulated([
  { name: 'Rx_Page_4.jpg', size: '1.1 MB', type: 'image/jpeg' },
  { name: 'Rx_Page_5.jpg', size: '1.5 MB', type: 'image/jpeg' },
  { name: 'Rx_Page_6.jpg', size: '1.3 MB', type: 'image/jpeg' }
]));

console.log("Total attached:", patientUploadedPrescriptions.length);
console.log("Names:", patientUploadedPrescriptions.map(p => p.name));
