//! Outbound transactional email — invite-to-connect links, Jira
//! usage-report notifications. Shared by both callers rather than each
//! building its own SMTP client.
//!
//! Disabled by default (`EmailConfig::enabled`); callers should check that
//! flag themselves and treat a disabled config as a no-op, not an error —
//! email is a notification nicety, never something core functionality
//! (invites, reports) should fail over.

use std::path::PathBuf;

use anyhow::{bail, Context};
use conductor_domain::EmailSettings;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::core::config::EmailConfig;
use crate::core::state::AppState;

fn require_configured(config: &EmailConfig) -> anyhow::Result<()> {
    if !config.enabled {
        bail!("email is not enabled (CONDUCTOR_EMAIL_ENABLED is not set)");
    }
    if config.smtp_host.is_empty() || config.from_address.is_empty() {
        bail!("email is enabled but CONDUCTOR_SMTP_HOST/CONDUCTOR_EMAIL_FROM are not set");
    }
    Ok(())
}

fn build_transport(
    config: &EmailConfig,
) -> anyhow::Result<AsyncSmtpTransport<Tokio1Executor>> {
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)
        .context("failed to configure SMTP relay")?
        .port(config.smtp_port);
    if !config.smtp_username.is_empty() {
        builder = builder.credentials(Credentials::new(
            config.smtp_username.clone(),
            config.smtp_password.clone(),
        ));
    }
    Ok(builder.build())
}

pub async fn send_email(
    config: &EmailConfig,
    to: &str,
    subject: &str,
    html_body: &str,
) -> anyhow::Result<()> {
    require_configured(config)?;

    let message = Message::builder()
        .from(
            config
                .from_address
                .parse()
                .context("invalid CONDUCTOR_EMAIL_FROM address")?,
        )
        .to(to.parse().context("invalid recipient email address")?)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html_body.to_string())
        .context("failed to build email message")?;

    build_transport(config)?
        .send(message)
        .await
        .context("failed to send email via SMTP")?;
    Ok(())
}

/// Same as `send_email`, with one file attached -- the report-export
/// delivery path. A separate function rather than an `Option` parameter on
/// `send_email`: every other caller sends a plain notification body and
/// should never need to think about attachment plumbing.
pub async fn send_email_with_attachment(
    config: &EmailConfig,
    to: &str,
    subject: &str,
    html_body: &str,
    attachment_filename: &str,
    attachment_content_type: &str,
    attachment_bytes: Vec<u8>,
) -> anyhow::Result<()> {
    require_configured(config)?;

    let content_type = ContentType::parse(attachment_content_type)
        .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
    let attachment = Attachment::new(attachment_filename.to_string())
        .body(attachment_bytes, content_type);

    let message = Message::builder()
        .from(
            config
                .from_address
                .parse()
                .context("invalid CONDUCTOR_EMAIL_FROM address")?,
        )
        .to(to.parse().context("invalid recipient email address")?)
        .subject(subject)
        .multipart(
            MultiPart::mixed()
                .singlepart(SinglePart::html(html_body.to_string()))
                .singlepart(attachment),
        )
        .context("failed to build email message")?;

    build_transport(config)?
        .send(message)
        .await
        .context("failed to send email via SMTP")?;
    Ok(())
}

fn smtp_password_path() -> PathBuf {
    let data_root = std::env::var("CONDUCTOR_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data"));
    data_root.join("secrets").join("smtp_password")
}

async fn write_smtp_password(bytes: &[u8]) -> anyhow::Result<()> {
    let path = smtp_password_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("create SMTP secret directory")?;
    }
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("SMTP password path must not be a symlink");
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("inspect SMTP password path"),
    }
    let temporary = path.with_extension(format!("conductor-{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&temporary, bytes)
        .await
        .context("write SMTP password")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
            .await
            .context("protect SMTP password")?;
    }
    tokio::fs::rename(&temporary, &path)
        .await
        .context("activate SMTP password")?;
    Ok(())
}

async fn read_smtp_password() -> anyhow::Result<Option<String>> {
    let path = smtp_password_path();
    match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("SMTP password path must not be a symlink");
        }
        Ok(_) => {
            let bytes = tokio::fs::read(&path).await.context("read SMTP password")?;
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("inspect SMTP password path"),
    }
}

async fn clear_smtp_password() -> anyhow::Result<()> {
    match tokio::fs::remove_file(smtp_password_path()).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("remove SMTP password"),
    }
}

/// Applies an `EmailSettings` update: persists the write-only password to its
/// secret file (or removes it), then returns settings safe to store in SQL
/// (`smtp_password` stripped, `smtp_password_set` reflecting the outcome).
pub async fn apply_email_settings_update(
    mut settings: EmailSettings,
) -> anyhow::Result<EmailSettings> {
    if settings.clear_smtp_password {
        clear_smtp_password().await?;
        settings.smtp_password_set = false;
    } else if let Some(password) = settings.smtp_password.take() {
        if !password.is_empty() {
            write_smtp_password(password.as_bytes()).await?;
            settings.smtp_password_set = true;
        }
    }
    settings.smtp_password = None;
    settings.clear_smtp_password = false;
    Ok(settings)
}

/// Resolves the SMTP configuration actually used at send time: DB-stored
/// settings when the operator has enabled and configured them from the
/// Settings UI, falling back to the environment-sourced [`EmailConfig`]
/// (`CONDUCTOR_SMTP_*`) otherwise — the "default optional" path.
pub async fn resolve_email_config(state: &AppState) -> anyhow::Result<EmailConfig> {
    let settings = state.db.instance().email_settings().await?;
    if !settings.enabled {
        return Ok(state.email.clone());
    }
    let password = read_smtp_password().await?.unwrap_or_default();
    Ok(EmailConfig {
        enabled: true,
        smtp_host: settings.smtp_host,
        smtp_port: settings.smtp_port,
        smtp_username: settings.smtp_username,
        smtp_password: password,
        from_address: settings.from_address,
    })
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Branded invite-to-connect email: an administrator invited `member_name`
/// to link their EvoFlux installation to `project_name`. Table-based layout
/// with inline styles, since that's what renders consistently across mail
/// clients (Gmail, Outlook) — not a general template engine.
pub fn invite_to_connect_email_body(
    project_name: &str,
    member_name: &str,
    connect_url: &str,
) -> String {
    let project_name = html_escape(project_name);
    let member_name = html_escape(member_name);
    let connect_url = html_escape(connect_url);
    format!(
        r#"<!doctype html>
<html>
  <body style="margin:0;padding:32px 16px;background:#f4f4f5;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;color:#18181b;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:480px;margin:0 auto;background:#ffffff;border-radius:12px;border:1px solid #e4e4e7;">
      <tr>
        <td style="padding:28px 32px 4px;">
          <p style="margin:0;font-size:12px;font-weight:600;letter-spacing:0.04em;text-transform:uppercase;color:#6366f1;">
            {project_name}
          </p>
        </td>
      </tr>
      <tr>
        <td style="padding:8px 32px 0;">
          <h1 style="margin:0 0 12px;font-size:20px;line-height:1.4;">You're invited to connect EvoFlux</h1>
          <p style="margin:0 0 20px;font-size:14px;line-height:1.6;color:#3f3f46;">
            Hi {member_name}, an administrator invited you to connect your EvoFlux
            installation to <strong>{project_name}</strong> on Conductor. Sign in
            once below to link your account — EvoFlux will then report usage
            under your own identity.
          </p>
        </td>
      </tr>
      <tr>
        <td style="padding:0 32px 24px;">
          <a href="{connect_url}" style="display:inline-block;padding:11px 22px;background:#111827;color:#ffffff;font-size:14px;font-weight:600;border-radius:8px;text-decoration:none;">
            Connect EvoFlux
          </a>
        </td>
      </tr>
      <tr>
        <td style="padding:0 32px 28px;">
          <p style="margin:0;font-size:12px;line-height:1.6;color:#71717a;">
            If the button doesn't work, copy this link into your browser:<br />
            <a href="{connect_url}" style="color:#6366f1;word-break:break-all;">{connect_url}</a>
          </p>
        </td>
      </tr>
      <tr>
        <td style="padding:16px 32px;border-top:1px solid #e4e4e7;background:#fafafa;border-radius:0 0 12px 12px;">
          <p style="margin:0;font-size:11px;color:#a1a1aa;">
            You're receiving this because an administrator invited this email
            address to {project_name} on Conductor. If you weren't expecting
            this, you can ignore it.
          </p>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    )
}

/// Renders a plain, unstyled HTML shell — callers own the actual content.
/// Kept intentionally minimal; this is not a template engine, it wraps a
/// pre-built body/CTA link in the smallest amount of HTML that reads
/// correctly across mail clients.
pub fn simple_email_body(intro_html: &str, cta_url: &str, cta_label: &str) -> String {
    format!(
        r#"<!doctype html>
<html>
  <body style="font-family: -apple-system, BlinkMacSystemFont, sans-serif; color: #1a1a1a;">
    <p>{intro_html}</p>
    <p>
      <a href="{cta_url}" style="display:inline-block;padding:10px 16px;background:#111827;color:#ffffff;border-radius:6px;text-decoration:none;">
        {cta_label}
      </a>
    </p>
    <p style="color:#6b7280;font-size:12px;">
      If the button doesn't work, copy this link: {cta_url}
    </p>
  </body>
</html>"#
    )
}
