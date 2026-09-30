const fs = require('fs');
const h = fs.readFileSync('prototype/index.html', 'utf8');
const lines = h.split('\n');

const funcs = [
  'function confirmAppointment',
  'function openDoctorSlotBooking',
  'function openPaidChatBookingForDoctor',
  'function openPaidChatBookingModal',
  'function confirmPaidChatBooking',
  'function executeAdminBatchPayout',
  'function approveDoctorBmdc'
];

lines.forEach((l, i) => {
  funcs.forEach(f => {
    if (l.includes(f)) {
      console.log((i + 1) + ': ' + l.trim());
    }
  });
});
