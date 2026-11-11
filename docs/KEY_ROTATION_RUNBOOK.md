# Key rotation runbook

This is a deployment checklist, not an automatic claim that rotation is
currently supported.

## JWT Ed25519

1. Generate a new key pair in the secret manager.
2. Add the new public key to the verifier trust set with a key identifier.
3. Switch signing to the new key.
4. Wait for the maximum access-token lifetime plus clock-skew allowance.
5. Remove the previous public key only after all old access tokens expire.
6. Revoke all refresh families if compromise is suspected.

Current implementation uses one configured key and has no `kid` overlap set;
this remains a production hardening item.

## Other secrets

Rotate DB role passwords, webhook secrets, Agora certificate, TOTP encryption
keys, storage credentials, and messaging credentials through the deployment
secret manager. Keep an overlap window only where the provider supports it,
and record the key version and rollback decision in the incident log.

**Exception: the pgBackRest backup-repository cipher passphrase does not
support this pattern.** It cannot be rotated in place — there is no overlap
window, because the old and new passphrases apply to two different
repositories, not two versions of the same one. See
`docs/BACKUP_ENCRYPTION_KEY_CUSTODY_RUNBOOK.md`'s "Key rotation
considerations" section for the correct new-repository migration procedure.
