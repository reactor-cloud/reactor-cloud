//! Email sender implementations.

use crate::config::SmtpConfig;
use async_trait::async_trait;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use reactor_core::auth::AuthError;

/// Trait for sending emails.
#[async_trait]
pub trait EmailSender: Send + Sync + 'static {
    /// Send an email.
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<(), AuthError>;

    /// Check if this sender is enabled (can actually send emails).
    fn is_enabled(&self) -> bool;
}

/// SMTP email sender using lettre.
pub struct SmtpSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpSender {
    /// Create a new SMTP sender from config.
    pub fn new(config: &SmtpConfig) -> Result<Self, AuthError> {
        let from: Mailbox = config.from.parse().map_err(|e| {
            tracing::error!(error = %e, "invalid SMTP from address");
            AuthError::Internal
        })?;

        let username = config.user.clone().unwrap_or_default();
        let password = config.password.clone().unwrap_or_default();
        let creds = Credentials::new(username, password);

        let transport = match config.tls.as_str() {
            // Direct TLS connection (implicit TLS on port 465)
            "tls" | "required" => {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                    .map_err(|e| {
                        tracing::error!(error = %e, "failed to create SMTP transport (TLS)");
                        AuthError::Internal
                    })?
                    .credentials(creds)
                    .port(config.port)
                    .build()
            }
            // STARTTLS connection (starts plain, upgrades to TLS)
            "starttls" | "opportunistic" => {
                AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                    .map_err(|e| {
                        tracing::error!(error = %e, "failed to create SMTP transport (STARTTLS)");
                        AuthError::Internal
                    })?
                    .credentials(creds)
                    .port(config.port)
                    .build()
            }
            // Plain connection (no TLS)
            _ => {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
                    .credentials(creds)
                    .port(config.port)
                    .build()
            }
        };

        Ok(Self { transport, from })
    }
}

#[async_trait]
impl EmailSender for SmtpSender {
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body_html: &str,
        body_text: &str,
    ) -> Result<(), AuthError> {
        let to_mailbox: Mailbox = to.parse().map_err(|e| {
            tracing::error!(error = %e, "invalid recipient email address");
            AuthError::ValidationError {
                message: "invalid email address".to_string(),
            }
        })?;

        // Create a multipart email with both HTML and text parts
        let email = Message::builder()
            .from(self.from.clone())
            .to(to_mailbox)
            .subject(subject)
            .multipart(
                lettre::message::MultiPart::alternative()
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(body_text.to_string()),
                    )
                    .singlepart(
                        lettre::message::SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(body_html.to_string()),
                    ),
            )
            .map_err(|e| {
                tracing::error!(error = %e, "failed to build email");
                AuthError::Internal
            })?;

        self.transport.send(email).await.map_err(|e| {
            tracing::error!(error = %e, "failed to send email");
            AuthError::Internal
        })?;

        Ok(())
    }

    fn is_enabled(&self) -> bool {
        true
    }
}

/// No-op email sender (for when SMTP is not configured).
#[derive(Clone)]
pub struct NoopSender;

#[async_trait]
impl EmailSender for NoopSender {
    async fn send(
        &self,
        to: &str,
        subject: &str,
        _body_html: &str,
        _body_text: &str,
    ) -> Result<(), AuthError> {
        tracing::warn!(
            to = to,
            subject = subject,
            "Email not sent (SMTP not configured)"
        );
        // Return success but log a warning - invitations still work via signed links
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        false
    }
}
