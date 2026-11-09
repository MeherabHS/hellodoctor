//! Operator-only, offline First-Admin bootstrap CLI.
//!
//! Not reachable over HTTP — this is a separate binary with no network
//! listener of its own. It is meant to be run once, interactively, by an
//! operator who already has shell/container access to the production VM:
//!
//!     docker compose --env-file /etc/hellodoctor/hellodoctor-vm.env \
//!         run --rm --no-deps -it backend /app/admin_bootstrap
//!
//! Every value this tool needs (DB host/port/name, the `hellodoctor_auth_worker`
//! role password, `ENVIRONMENT`) comes from the exact same `AppConfig::from_env()`
//! the real server reads — never a parallel/duplicated config path, so there is
//! no way for this tool to connect anywhere the real backend doesn't. All of
//! the actual logic (duplicate checks, locking, insertion, audit) lives in
//! `hellodoctor_backend::services::admin_bootstrap_service`, which is what the
//! automated test suite exercises directly — this binary only collects input
//! from the operator and reports the result.
//!
//! The Admin created here authenticates through the ordinary production
//! `/auth/admin/login` -> email-OTP flow exactly like any other Admin; this
//! tool never bypasses, weakens, or special-cases that path.
use hellodoctor_backend::config::AppConfig;
use hellodoctor_backend::services::admin_bootstrap_service::bootstrap_first_admin;
use sqlx::postgres::PgPoolOptions;
use std::io::{self, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Runtime::new()?.block_on(run())
}

fn prompt_line(label: &str) -> Result<String, Box<dyn std::error::Error>> {
    print!("{label}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Reuses the real server's own config loader and validation — if this
    // panics on a missing/invalid production secret, that is the correct
    // behavior: this tool must never proceed against a database it cannot
    // describe exactly as the real backend would.
    let config = AppConfig::from_env();

    println!("== HelloDoctor.Asia — First-Admin Bootstrap ==");
    println!(
        "Target database : {}:{}/{}",
        config.app_db_host, config.app_db_port, config.app_db_name
    );
    println!("Environment     : {}", config.environment);
    println!();
    println!("This tool creates the very first Admin account. It refuses to run if any");
    println!("Admin account already exists, or if the email/phone given is already in use.");
    println!();

    let confirm_target = prompt_line(&format!(
        "Type the database name ('{}') to confirm you intend to write to THIS database: ",
        config.app_db_name
    ))?;
    if confirm_target != config.app_db_name {
        return Err(
            "Confirmation did not match the target database name. Aborting; nothing was written."
                .into(),
        );
    }

    let email = prompt_line("Admin email: ")?;
    let email_confirm = prompt_line("Re-enter the same email to confirm: ")?;
    if email != email_confirm {
        return Err("Email confirmation did not match. Aborting; nothing was written.".into());
    }

    let phone_number = prompt_line("Admin phone number (e.g. +2547XXXXXXXX): ")?;

    let password = rpassword::prompt_password("New Admin password: ")?;
    let password_confirm = rpassword::prompt_password("Confirm new Admin password: ")?;
    if password != password_confirm {
        return Err("Passwords did not match. Aborting; nothing was written.".into());
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.auth_worker_dsn())
        .await?;

    let result = bootstrap_first_admin(&pool, &email, &phone_number, &password).await;
    drop(password);
    drop(password_confirm);

    match result {
        Ok(outcome) => {
            println!();
            println!("Admin account created (user_id={}).", outcome.user_id);
            println!(
                "Sign in at the normal Admin Web login — email OTP will be required as usual."
            );
            Ok(())
        }
        Err(error) => Err(format!("Bootstrap refused: {error}").into()),
    }
}
