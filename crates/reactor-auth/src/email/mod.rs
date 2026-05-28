//! Email transport module using lettre for SMTP.

mod sender;
mod templates;

pub use sender::{EmailSender, NoopSender, SmtpSender};
pub use templates::EmailTemplate;
