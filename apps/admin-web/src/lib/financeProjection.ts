import type {TransactionItem} from '../data/adminStore';

export function backendTransactionToStoreItem(tx: any): TransactionItem {
  const states: Record<string, Pick<TransactionItem, 'type' | 'typeLabel' | 'direction' | 'escrowStatus' | 'statusBadgeCls'>> = {
    INITIATED: {type:'INFLOW',typeLabel:'Unconfirmed checkout',direction:'NONE',escrowStatus:'UNFUNDED',statusBadgeCls:'badge-amber'},
    FAILED: {type:'INFLOW',typeLabel:'Unfunded checkout closed',direction:'NONE',escrowStatus:'UNFUNDED',statusBadgeCls:'badge-gray'},
    PAYMENT_HELD: {type:'ESCROW',typeLabel:'Confirmed payment held',direction:'INFLOW',escrowStatus:'ESCROW_LOCKED',statusBadgeCls:'badge-amber'},
    REFUND_PENDING: {type:'REFUND',typeLabel:'Refund approved; completion pending',direction:'NONE',escrowStatus:'ESCROW_LOCKED',statusBadgeCls:'badge-amber'},
    SETTLED_TO_DOCTOR: {type:'INFLOW',typeLabel:'Consultation settled',direction:'INFLOW',escrowStatus:'SETTLED',statusBadgeCls:'badge-green'},
    DISBURSED: {type:'PAYOUT',typeLabel:'Doctor payout completed',direction:'PAYOUT',escrowStatus:'SETTLED',statusBadgeCls:'badge-green'},
    REFUNDED_TO_PATIENT: {type:'REFUND',typeLabel:'Refund completed',direction:'REFUND',escrowStatus:'REFUNDED',statusBadgeCls:'badge-green'},
  };
  if (!Object.hasOwn(states, tx.payment_status)) throw new Error('Unsupported payment state');
  const state = states[tx.payment_status];
  const gross = Number(tx.gross_amount || 0);
  const fee = Number(tx.platform_fee_amount || 0);
  const net = Number(tx.net_amount || 0);

  return {
    txId: tx.transaction_number || tx.id,
    gatewayRef: tx.gateway_reference || 'No provider reference',
    timestamp: tx.created_at ? new Date(tx.created_at).toLocaleString() : 'Today',
    consultationId: tx.appointment_id || 'CONS',
    sessionType: 'Video Consultation',
    type: state.type,
    typeLabel: state.typeLabel,
    gateway: tx.gateway === 'MPESA' ? 'MFS Bulk API' : tx.gateway === 'CARD' ? 'Card' : 'MFS Bulk API',
    gatewayLabel: tx.gateway === 'MPESA' ? 'M-Pesa provider' : tx.gateway === 'BANK_TRANSFER' ? 'Kenyan bank provider' : tx.gateway === 'CARD' ? 'Card provider' : 'Provider deferred',
    patient: {
      name: tx.patient_name || 'Patient',
      phone: 'Not provided',
      id: 'USR',
      avatar: 'PT',
      avatarBg: '#2563EB',
    },
    doctor: {
      name: tx.doctor_name || 'Physician',
      registrationNumber: tx.doctor_license || 'Not provided',
      specialty: 'Specialist',
      avatar: 'DR',
      avatarBg: '#059669',
    },
    grossAmount: gross,
    platformFeePercent: 20,
    platformFeeAmount: fee,
    netAmount: net,
    direction: state.direction,
    escrowStatus: state.escrowStatus,
    escrowStatusLabel: tx.reconciliation_required ? `${tx.payment_status} — reconciliation required` : tx.payment_status,
    statusBadgeCls: state.statusBadgeCls,
    gatewayPayload: { tx_number: tx.transaction_number },
    auditTrail: [{ time: 'Recent', event: state.typeLabel }],
  };
}
