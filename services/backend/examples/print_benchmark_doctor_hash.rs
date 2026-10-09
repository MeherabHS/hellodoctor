//! One-off: prints a real Argon2id hash of the fixed, documented
//! benchmark-only Doctor password used by the auth-stress seed data and
//! examples/auth_stress_test.rs. Never a real credential — run once, paste
//! the printed hash into scripts/seed_auth_stress_doctors.sql, do not wire
//! this into any runtime path.
fn main() {
    let password = "AuthStressBenchmark-2026!";
    let hash =
        hellodoctor_backend::crypto::hash_password(password).expect("hash benchmark password");
    println!("{hash}");
}
