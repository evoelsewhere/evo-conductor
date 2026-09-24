//! Ad hoc: send one real email through the exact `resolve_email_config` +
//! `send_email` path, using whatever is stored in `data/conductor.db` /
//! `data/secrets/smtp_password`. Prints the real transport error instead of
//! going through the full HTTP/auth stack.
//!
//! Usage: cargo run -p conductor-server --example send_test_email -- <to-address>

use conductor_domain::EmailSettings;
use conductor_storage::repos::InstanceRepo;
use sqlx::any::AnyPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let to = std::env::args()
        .nth(1)
        .expect("usage: send_test_email <to-address>");

    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite:data/conductor.db")
        .await?;
    let instance = InstanceRepo::new(pool);
    let settings: EmailSettings = instance.email_settings().await?;
    println!(
        "DB email settings: enabled={} host={} port={} username={} from={} password_set={}",
        settings.enabled,
        settings.smtp_host,
        settings.smtp_port,
        settings.smtp_username,
        settings.from_address,
        settings.smtp_password_set
    );

    let password_path = std::path::Path::new("data/secrets/smtp_password");
    let password = std::fs::read_to_string(password_path)
        .map(|value| value.trim().to_string())
        .unwrap_or_default();
    println!(
        "Secret file {}: {} bytes",
        password_path.display(),
        password.len()
    );

    let config = conductor_server::EmailConfig {
        enabled: settings.enabled,
        smtp_host: settings.smtp_host,
        smtp_port: settings.smtp_port,
        smtp_username: settings.smtp_username,
        smtp_password: password,
        from_address: settings.from_address,
    };

    let body = conductor_server::core::email::simple_email_body(
        "This is a test email from Conductor's Email settings tab.",
        "https://example.com",
        "Example link",
    );

    match conductor_server::core::email::send_email(
        &config,
        &to,
        "Conductor test email",
        &body,
    )
    .await
    {
        Ok(()) => println!("Sent OK to {to}"),
        Err(error) => println!("Send FAILED: {error:#}"),
    }

    Ok(())
}
