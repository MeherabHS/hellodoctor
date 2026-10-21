UPDATE disbursement_items SET status='PAID',gateway_reference='SYNTHETIC-PAYOUT-REFERENCE',paid_at=now()
WHERE id='aaaaaaaa-6666-6666-6666-666666666666' AND status IN ('APPROVED','PROCESSING');
