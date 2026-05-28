//! Custom domain management.

pub mod verify;

#[cfg(feature = "domain-acme")]
pub mod acme;


/// Domain verification instructions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VerificationInstructions {
    /// Verification method: "dns" or "http".
    pub method: String,
    /// Verification token.
    pub token: String,
    /// DNS record to create (for DNS verification).
    pub dns_record: Option<DnsRecord>,
    /// HTTP path to serve token (for HTTP verification).
    pub http_path: Option<String>,
}

/// DNS record for domain verification.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DnsRecord {
    /// Record type (always TXT).
    pub record_type: String,
    /// Record name (e.g., "_reactor-verify.app.example.com").
    pub name: String,
    /// Record value.
    pub value: String,
}

/// Generate verification instructions for a domain.
pub fn generate_verification_instructions(
    host: &str,
    token: &str,
    method: &str,
) -> VerificationInstructions {
    match method {
        "dns" => VerificationInstructions {
            method: "dns".to_string(),
            token: token.to_string(),
            dns_record: Some(DnsRecord {
                record_type: "TXT".to_string(),
                name: format!("_reactor-verify.{}", host),
                value: format!("reactor-site-verification={}", token),
            }),
            http_path: None,
        },
        "http" => VerificationInstructions {
            method: "http".to_string(),
            token: token.to_string(),
            dns_record: None,
            http_path: Some(format!(
                "http://{}/.well-known/reactor-verify",
                host
            )),
        },
        _ => VerificationInstructions {
            method: method.to_string(),
            token: token.to_string(),
            dns_record: None,
            http_path: None,
        },
    }
}
