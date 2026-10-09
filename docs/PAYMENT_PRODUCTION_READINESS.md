# Payment production readiness

| Control | Status |
|---|---|
| Internal PAYMENT_HELD -> settlement state machine | IMPLEMENTED_AND_TESTED |
| Wallet settlement exactly-once guard | IMPLEMENTED_AND_TESTED |
| Disbursement claim exactly-once guard | IMPLEMENTED_AND_TESTED |
| Signed/fresh webhook envelope | IMPLEMENTED_AND_TESTED |
| Provider-to-transaction gateway binding | IMPLEMENTED_AND_TESTED |
| Provider-specific amount/currency/merchant validation | DEFERRED_EXTERNAL_DEPENDENCY |
| UNKNOWN payment status and reconciliation worker | NOT_IMPLEMENTED |
| Abandoned-payment expiry and slot release worker | NOT_IMPLEMENTED |
| Provider refund execution/status query | DEFERRED_EXTERNAL_DEPENDENCY |
| Provider payout execution/status query | DEFERRED_EXTERNAL_DEPENDENCY |
| Currency-separated ledger/reporting | NOT_IMPLEMENTED |
| Sandbox end-to-end tests | DEFERRED_EXTERNAL_DEPENDENCY |

No provider integration is fabricated by this document. Real-money enablement
requires provider adapters, sandbox tests, reconciliation, and finance signoff.
