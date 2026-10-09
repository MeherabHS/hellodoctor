-- Keep Kenyan bank-account payout routing distinct from historical mobile
-- money values. Existing BKASH/NAGAD records remain readable for migration
-- compatibility but are not active Kenya payout options.
ALTER TYPE gateway_enum ADD VALUE IF NOT EXISTS 'BANK_TRANSFER';
