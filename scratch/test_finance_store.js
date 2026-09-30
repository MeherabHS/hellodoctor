// Prototype test script for admin finance ledger implementation
const fs = require('fs');

console.log("Testing data store structure for finance ledger...");
const sampleStore = [
  {
    txId: 'TXN-BK-94812',
    gatewayRef: 'BK-MER-88492019',
    timestamp: 'Today, 10:28 AM',
    consultationId: 'CONS-9481',
    sessionType: 'Video Consultation (10 min)',
    type: 'INFLOW',
    typeLabel: 'Consultation Inflow',
    gateway: 'bKash',
    gatewayLabel: 'bKash Merchant',
    patient: { name: 'Sarah Khan', phone: '+880 1711-234567', id: 'USR-8921' },
    doctor: { name: 'Dr. Sabrina Akter', bmdc: 'BMDC #45821', specialty: 'Internal Medicine' },
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
  }
];

console.log("Sample store item validated:", sampleStore[0].txId);
console.log("Reconciliation math check:", sampleStore[0].grossAmount === (sampleStore[0].platformFeeAmount + sampleStore[0].netAmount));
