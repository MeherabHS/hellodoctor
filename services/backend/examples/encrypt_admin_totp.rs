//! Owner-only offline enrollment helper. Reads a Base32 secret from stdin;
//! prints ciphertext only. Never execute with production credentials in tests.
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("TOTP_ENCRYPTION_KEY_B64")?;
    let mut secret = String::new();
    io::stdin().take(256).read_to_string(&mut secret)?;
    let secret = secret.trim();
    hellodoctor_backend::services::auth_service::admin_totp::decode_base32(secret)?;
    let encrypted = hellodoctor_backend::crypto::encrypt_secret(secret, &key)?;
    println!("{}", hex::encode(encrypted));
    Ok(())
}
