use crate::domain::ids::UserId;
use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct AgoraTokenResponse {
    pub channel_name: String,
    pub token: String,
    pub uid: u32,
    pub app_id: String,
    pub expires_in: u32,
}

pub struct AgoraTokenService;

/// Used when a caller doesn't have a tighter, window-derived budget to pass
/// (`ConsultationService::mint_agora_token` always does — see its own
/// comment). Matches this service's previous hardcoded `expires_in`.
pub const DEFAULT_TOKEN_EXPIRE_SECS: u32 = 3600;

const VERSION: &str = "007";
const RTC_SERVICE_TYPE: u16 = 1;
const PRIVILEGE_JOIN_CHANNEL: u16 = 1;
const PRIVILEGE_PUBLISH_AUDIO_STREAM: u16 = 2;
const PRIVILEGE_PUBLISH_VIDEO_STREAM: u16 = 3;
const PRIVILEGE_PUBLISH_DATA_STREAM: u16 = 4;

impl AgoraTokenService {
    /// Mints a real Agora AccessToken2 (the "007" token format Agora's SDKs
    /// and edge servers actually validate — ported byte-for-byte from
    /// Agora's own reference implementation, `AgoraIO/Tools`,
    /// `DynamicKey/AgoraDynamicKey/python3/src/{AccessToken2,Packer}.py`; see
    /// `build_rtc_access_token2`'s own comment for the exact wire format).
    /// The App Certificate never leaves this process (CLAUDE.md security
    /// rule) — it is read from server-only config and used solely as HMAC
    /// key material here.
    ///
    /// `expire_secs` bounds both the token's own validity and every RTC
    /// privilege (join/publish audio/publish video/publish data) granted by
    /// it — callers should pass the real remaining authorization-window
    /// budget (see `ConsultationService::mint_agora_token`) rather than a
    /// fixed constant, so a token can never outlive the appointment window
    /// that justified minting it.
    pub fn mint(
        channel_name: &str,
        user_id: UserId,
        agora_app_id: &str,
        agora_certificate: &str,
        expire_secs: u32,
    ) -> Result<AgoraTokenResponse, AppError> {
        validate_credential_format("AGORA_APP_ID", agora_app_id)?;
        validate_credential_format("AGORA_APP_CERTIFICATE", agora_certificate)?;
        let uid = derive_uid(channel_name, user_id);
        let token = build_rtc_access_token2(
            agora_app_id,
            agora_certificate,
            channel_name,
            uid,
            expire_secs,
        )?;
        Ok(AgoraTokenResponse {
            channel_name: channel_name.to_string(),
            token,
            uid,
            app_id: agora_app_id.to_string(),
            expires_in: expire_secs,
        })
    }
}

/// Agora's own reference implementation refuses to build a token unless both
/// identifiers are exactly 32 hex characters (the real App ID/Certificate
/// format) — ported here as an explicit, loud failure rather than silently
/// minting a token Agora's servers would reject. A missing env var already
/// fails closed at process startup (`config.rs`'s `require()`); this is the
/// second, independent guard for "present but malformed" — e.g. a copy-paste
/// error or an unconfigured placeholder — which `require()` alone can't catch.
fn validate_credential_format(name: &str, value: &str) -> Result<(), AppError> {
    if value.len() != 32 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(AppError::internal(format!(
            "{name} is not configured in Agora's required 32-character hexadecimal format — refusing to mint a token Agora would reject."
        )));
    }
    Ok(())
}

/// Derives a deterministic, collision-resistant Agora UID (0..=u32::MAX)
/// from both the channel name and the caller's real user id — the Pass 2
/// fix for a real bug: the previous scheme (`user_id.as_u128() % 100_000`)
/// collapsed a 128-bit UUID into a 5-digit range, giving the *two*
/// legitimate participants of the same call a real (if small) chance of
/// colliding onto the same Agora UID. Hashing in the channel name too means
/// the derivation is still stable/deterministic per (channel, user) — the
/// same caller reconnecting to the same channel always gets the same UID —
/// while two different users' hashes within one channel are independent,
/// ~1-in-4-billion-collision-odds values rather than 1-in-100,000. Doesn't
/// need a new table: only two participants ever share a given channel, so a
/// full 32-bit hash is already a huge margin for that birthday problem.
pub fn derive_uid(channel_name: &str, user_id: UserId) -> u32 {
    let mut hasher = Sha256::new();
    hasher.update(channel_name.as_bytes());
    hasher.update(user_id.into_inner().as_bytes());
    let digest = hasher.finalize();
    let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    // 0 has a special "let the server auto-assign a UID" meaning in some
    // Agora SDK integration modes — avoid ever producing it here.
    if raw == 0 {
        1
    } else {
        raw
    }
}

// ============================================================================
// Agora AccessToken2 ("007" token) — a faithful, byte-for-byte port of
// Agora's own open-source reference implementation (`AgoraIO/Tools`,
// `DynamicKey/AgoraDynamicKey/python3/src/AccessToken2.py` + `Packer.py`,
// fetched and verified against the published source, not reconstructed from
// memory). Only the RTC service is implemented — the only one this product
// needs — but the generic packing primitives and signing algorithm are
// exactly Agora's own, so this interoperates with the real Agora SDK/edge
// servers without modification on their side.
//
// Wire format, most-significant facts first:
//   token = "007" + base64_standard(zlib_compress(pack_string(signature) ++ signing_info))
//   signing_info = pack_string(app_id) ++ u32_le(issue_ts) ++ u32_le(expire)
//                  ++ u32_le(salt) ++ u16_le(service_count=1) ++ service
//   service (RTC, type=1) = u16_le(1) ++ pack_map_u32(privileges, sorted by key)
//                           ++ pack_string(channel_name) ++ pack_string(uid_as_decimal_string_or_empty_if_0)
//   pack_string(s)  = u16_le(len(s)) ++ s                      (UTF-8 bytes, or raw bytes for the signature)
//   pack_map_u32(m) = u16_le(len(m)) ++ for (k,v) sorted by k: u16_le(k) ++ u32_le(v)
//   signing_key = HMAC-SHA256(key=u32_le(salt), msg=HMAC-SHA256(key=u32_le(issue_ts), msg=app_certificate))
//   signature   = HMAC-SHA256(key=signing_key, msg=signing_info)
// Both the token's own `expire` and every privilege's `expire` are relative
// second-durations from `issue_ts` (Agora's own default constructor value is
// 900, a plausible duration, never a Unix timestamp) — evaluated by Agora's
// servers as `issue_ts + expire`, not computed here.
// ============================================================================

fn pack_u16_into(buf: &mut Vec<u8>, x: u16) {
    buf.extend_from_slice(&x.to_le_bytes());
}

fn pack_u32_into(buf: &mut Vec<u8>, x: u32) {
    buf.extend_from_slice(&x.to_le_bytes());
}

fn pack_bytes_into(buf: &mut Vec<u8>, b: &[u8]) {
    pack_u16_into(buf, b.len() as u16);
    buf.extend_from_slice(b);
}

fn pack_string_into(buf: &mut Vec<u8>, s: &str) {
    pack_bytes_into(buf, s.as_bytes());
}

fn pack_map_u32_into(buf: &mut Vec<u8>, m: &BTreeMap<u16, u32>) {
    // `BTreeMap` iterates in ascending key order already, matching Agora's
    // own `sorted(items, key=lambda x: int(x[0]))` packing order exactly.
    pack_u16_into(buf, m.len() as u16);
    for (k, v) in m {
        pack_u16_into(buf, *k);
        pack_u32_into(buf, *v);
    }
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    // `Hmac::new_from_slice` only rejects a key by length for MACs with a
    // fixed required key size; HMAC accepts any key length by construction.
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().into()
}

fn rtc_signing_key(issue_ts: u32, salt: u32, app_certificate: &str) -> [u8; 32] {
    let mut issue_ts_bytes = Vec::with_capacity(4);
    pack_u32_into(&mut issue_ts_bytes, issue_ts);
    let k1 = hmac_sha256(&issue_ts_bytes, app_certificate.as_bytes());
    let mut salt_bytes = Vec::with_capacity(4);
    pack_u32_into(&mut salt_bytes, salt);
    hmac_sha256(&salt_bytes, &k1)
}

fn pack_rtc_service(channel_name: &str, uid: u32, expire_secs: u32) -> Vec<u8> {
    let mut privileges = BTreeMap::new();
    privileges.insert(PRIVILEGE_JOIN_CHANNEL, expire_secs);
    privileges.insert(PRIVILEGE_PUBLISH_AUDIO_STREAM, expire_secs);
    privileges.insert(PRIVILEGE_PUBLISH_VIDEO_STREAM, expire_secs);
    privileges.insert(PRIVILEGE_PUBLISH_DATA_STREAM, expire_secs);

    let mut service = Vec::new();
    pack_u16_into(&mut service, RTC_SERVICE_TYPE);
    pack_map_u32_into(&mut service, &privileges);
    pack_string_into(&mut service, channel_name);
    // Agora's own reference packs the uid as its decimal string form, empty
    // string meaning "wildcard" (uid 0) — never the raw 4 bytes of the u32.
    let uid_str = if uid == 0 {
        String::new()
    } else {
        uid.to_string()
    };
    pack_string_into(&mut service, &uid_str);
    service
}

fn build_rtc_access_token2(
    app_id: &str,
    app_certificate: &str,
    channel_name: &str,
    uid: u32,
    expire_secs: u32,
) -> Result<String, AppError> {
    let issue_ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::internal(format!("system clock before UNIX epoch: {e}")))?
        .as_secs() as u32;
    // Agora's own reference uses `secrets.SystemRandom().randint(1, 99999999)`
    // — any per-token unpredictable salt in that range is equivalent; this
    // process's CSPRNG (`rand::thread_rng`) is the Rust-idiomatic match.
    let salt: u32 = rand::thread_rng().gen_range(1..=99_999_999);

    let service = pack_rtc_service(channel_name, uid, expire_secs);

    let mut signing_info = Vec::new();
    pack_string_into(&mut signing_info, app_id);
    pack_u32_into(&mut signing_info, issue_ts);
    pack_u32_into(&mut signing_info, expire_secs);
    pack_u32_into(&mut signing_info, salt);
    pack_u16_into(&mut signing_info, 1); // exactly one service: RTC
    signing_info.extend_from_slice(&service);

    let signing_key = rtc_signing_key(issue_ts, salt, app_certificate);
    let signature = hmac_sha256(&signing_key, &signing_info);

    let mut payload = Vec::new();
    pack_bytes_into(&mut payload, &signature);
    payload.extend_from_slice(&signing_info);

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&payload)
        .map_err(|e| AppError::internal(format!("Agora token zlib compression failed: {e}")))?;
    let compressed = encoder
        .finish()
        .map_err(|e| AppError::internal(format!("Agora token zlib compression failed: {e}")))?;

    Ok(format!("{VERSION}{}", STANDARD.encode(compressed)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use std::io::Read as _;

    #[test]
    fn derive_uid_is_deterministic() {
        let user_id = UserId::new();
        let a = derive_uid("rtc_channel_1", user_id);
        let b = derive_uid("rtc_channel_1", user_id);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_uid_differs_across_channels_for_same_user() {
        let user_id = UserId::new();
        let a = derive_uid("rtc_channel_1", user_id);
        let b = derive_uid("rtc_channel_2", user_id);
        assert_ne!(a, b);
    }

    #[test]
    fn derive_uid_differs_across_users_in_same_channel() {
        let a = derive_uid("rtc_channel_1", UserId::new());
        let b = derive_uid("rtc_channel_1", UserId::new());
        assert_ne!(a, b);
    }

    // ------------------------------------------------------------------
    // Test-only AccessToken2 *decoder* — the exact inverse of the packing
    // functions above. It exists purely so these tests can assert on the
    // real decoded structure of a minted token (not just "it's a non-empty
    // string"), and so the signature can be independently re-derived and
    // compared, which is what actually proves the signing algorithm and the
    // packed byte layout agree with each other byte-for-byte.
    // ------------------------------------------------------------------
    struct DecodedRtcToken {
        signature: Vec<u8>,
        signing_info: Vec<u8>,
        app_id: String,
        issue_ts: u32,
        expire: u32,
        salt: u32,
        service_count: u16,
        service_type: u16,
        privileges: BTreeMap<u16, u32>,
        channel_name: String,
        uid: String,
    }

    fn unpack_u16(buf: &[u8]) -> (u16, &[u8]) {
        (u16::from_le_bytes([buf[0], buf[1]]), &buf[2..])
    }
    fn unpack_u32(buf: &[u8]) -> (u32, &[u8]) {
        (
            u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            &buf[4..],
        )
    }
    fn unpack_bytes(buf: &[u8]) -> (Vec<u8>, &[u8]) {
        let (len, rest) = unpack_u16(buf);
        let len = len as usize;
        (rest[..len].to_vec(), &rest[len..])
    }
    fn unpack_string(buf: &[u8]) -> (String, &[u8]) {
        let (bytes, rest) = unpack_bytes(buf);
        (String::from_utf8(bytes).unwrap(), rest)
    }
    fn unpack_map_u32(buf: &[u8]) -> (BTreeMap<u16, u32>, &[u8]) {
        let (count, mut rest) = unpack_u16(buf);
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let (k, r1) = unpack_u16(rest);
            let (v, r2) = unpack_u32(r1);
            map.insert(k, v);
            rest = r2;
        }
        (map, rest)
    }

    fn decode_rtc_token(token: &str) -> DecodedRtcToken {
        assert_eq!(
            &token[..3],
            VERSION,
            "token must start with the AccessToken2 version prefix"
        );
        let compressed = STANDARD.decode(&token[3..]).expect("valid base64 payload");
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut payload = Vec::new();
        decoder
            .read_to_end(&mut payload)
            .expect("valid zlib-compressed payload");

        let (signature, rest) = unpack_bytes(&payload);
        let signing_info = rest.to_vec();
        let (app_id, rest) = unpack_string(rest);
        let (issue_ts, rest) = unpack_u32(rest);
        let (expire, rest) = unpack_u32(rest);
        let (salt, rest) = unpack_u32(rest);
        let (service_count, rest) = unpack_u16(rest);
        let (service_type, rest) = unpack_u16(rest);
        let (privileges, rest) = unpack_map_u32(rest);
        let (channel_name, rest) = unpack_string(rest);
        let (uid, rest) = unpack_string(rest);
        assert!(
            rest.is_empty(),
            "no trailing bytes after the single RTC service"
        );

        DecodedRtcToken {
            signature,
            signing_info,
            app_id,
            issue_ts,
            expire,
            salt,
            service_count,
            service_type,
            privileges,
            channel_name,
            uid,
        }
    }

    const TEST_APP_ID: &str = "73ddf1abee12ac89822eecb0e7df9e28";
    const TEST_APP_CERTIFICATE: &str = "12a7c64e686cb880ee5cb57ac9eed39d";

    #[test]
    fn mint_produces_a_structurally_valid_access_token2() {
        let channel = "rtc_structural_test_channel";
        let uid = 424_242u32;
        let expire_secs = 1800u32;
        let token =
            build_rtc_access_token2(TEST_APP_ID, TEST_APP_CERTIFICATE, channel, uid, expire_secs)
                .unwrap();
        let decoded = decode_rtc_token(&token);

        assert_eq!(decoded.app_id, TEST_APP_ID);
        assert_eq!(decoded.expire, expire_secs);
        assert_eq!(decoded.service_count, 1);
        assert_eq!(decoded.service_type, RTC_SERVICE_TYPE);
        assert_eq!(decoded.channel_name, channel);
        assert_eq!(decoded.uid, uid.to_string());
        assert_eq!(
            decoded.privileges,
            BTreeMap::from([
                (PRIVILEGE_JOIN_CHANNEL, expire_secs),
                (PRIVILEGE_PUBLISH_AUDIO_STREAM, expire_secs),
                (PRIVILEGE_PUBLISH_VIDEO_STREAM, expire_secs),
                (PRIVILEGE_PUBLISH_DATA_STREAM, expire_secs),
            ]),
            "must grant exactly join + publish audio/video/data, nothing more and nothing less"
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        assert!(
            decoded.issue_ts.abs_diff(now) <= 5,
            "issue_ts must be the real current time, not a placeholder"
        );
    }

    #[test]
    fn mint_wildcard_uid_packs_an_empty_uid_string() {
        // uid=0 is Agora's own "let the server auto-assign" wildcard — the
        // reference implementation packs it as an empty string, never "0".
        let token =
            build_rtc_access_token2(TEST_APP_ID, TEST_APP_CERTIFICATE, "rtc_wildcard", 0, 900)
                .unwrap();
        let decoded = decode_rtc_token(&token);
        assert_eq!(decoded.uid, "");
    }

    #[test]
    fn mint_signature_verifies_against_the_real_certificate() {
        let token = build_rtc_access_token2(
            TEST_APP_ID,
            TEST_APP_CERTIFICATE,
            "rtc_signature_test",
            7,
            900,
        )
        .unwrap();
        let decoded = decode_rtc_token(&token);
        // Re-derive the signature completely independently from the decoded
        // signing_info and the known-correct certificate, and require an
        // exact match — this is the real cryptographic correctness check:
        // it fails if the signing-key derivation, the HMAC message, or the
        // packed byte layout disagree with each other in any way.
        let signing_key = rtc_signing_key(decoded.issue_ts, decoded.salt, TEST_APP_CERTIFICATE);
        let expected_signature = hmac_sha256(&signing_key, &decoded.signing_info);
        assert_eq!(decoded.signature, expected_signature.to_vec());
    }

    #[test]
    fn mint_signature_does_not_verify_against_the_wrong_certificate() {
        let token = build_rtc_access_token2(
            TEST_APP_ID,
            TEST_APP_CERTIFICATE,
            "rtc_wrong_cert_test",
            7,
            900,
        )
        .unwrap();
        let decoded = decode_rtc_token(&token);
        let wrong_certificate = "ffffffffffffffffffffffffffffffff";
        let signing_key = rtc_signing_key(decoded.issue_ts, decoded.salt, wrong_certificate);
        let forged_signature = hmac_sha256(&signing_key, &decoded.signing_info);
        assert_ne!(
            decoded.signature,
            forged_signature.to_vec(),
            "the App Certificate must be real, load-bearing key material, not decorative"
        );
    }

    #[test]
    fn mint_is_nondeterministic_across_calls_same_inputs() {
        // Same inputs, two calls: issue_ts/salt differ (real time + a fresh
        // random salt each time), so a captured token can never be replayed
        // as a stand-in for a fresh one, and two tokens for the same
        // (app, channel, uid) are never byte-identical.
        let a = build_rtc_access_token2(TEST_APP_ID, TEST_APP_CERTIFICATE, "rtc_replay", 1, 900)
            .unwrap();
        let b = build_rtc_access_token2(TEST_APP_ID, TEST_APP_CERTIFICATE, "rtc_replay", 1, 900)
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn mint_rejects_malformed_app_id() {
        let err = AgoraTokenService::mint(
            "rtc_bad_app_id",
            UserId::new(),
            "not-32-hex-chars",
            TEST_APP_CERTIFICATE,
            DEFAULT_TOKEN_EXPIRE_SECS,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn mint_rejects_malformed_app_certificate() {
        let err = AgoraTokenService::mint(
            "rtc_bad_cert",
            UserId::new(),
            TEST_APP_ID,
            "short",
            DEFAULT_TOKEN_EXPIRE_SECS,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn mint_end_to_end_response_matches_the_token_it_minted() {
        let user_id = UserId::new();
        let resp = AgoraTokenService::mint(
            "rtc_end_to_end",
            user_id,
            TEST_APP_ID,
            TEST_APP_CERTIFICATE,
            1200,
        )
        .unwrap();
        assert_eq!(resp.channel_name, "rtc_end_to_end");
        assert_eq!(resp.app_id, TEST_APP_ID);
        assert_eq!(resp.expires_in, 1200);
        assert_eq!(resp.uid, derive_uid("rtc_end_to_end", user_id));
        let decoded = decode_rtc_token(&resp.token);
        assert_eq!(decoded.uid, resp.uid.to_string());
        assert_eq!(decoded.expire, 1200);
    }
}
