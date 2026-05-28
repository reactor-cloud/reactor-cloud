//! Event enrichment pipeline.
//!
//! Enriches incoming events with:
//! - IP address truncation (/24 IPv4, /48 IPv6)
//! - User agent parsing (browser, device type)
//! - Geo lookup (country from MaxMind)
//! - Referrer host extraction
//! - UTM parameter extraction
//! - Denied property stripping

use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

use maxminddb::{geoip2, Reader};
use url::Url;
use woothee::parser::Parser as UaParser;

use super::{ClientContext, IngestEvent, UtmContext};
use crate::config::AnalyticsConfig;

/// Enricher for incoming analytics events.
pub struct Enricher {
    /// User agent parser.
    ua_parser: UaParser,
    /// MaxMind GeoLite2 country database reader.
    geo_reader: Option<Reader<Vec<u8>>>,
    /// Configuration.
    config: Arc<AnalyticsConfig>,
}

impl Enricher {
    /// Create a new enricher.
    pub fn new(config: Arc<AnalyticsConfig>) -> Self {
        let geo_reader = config.geo_db_path.as_ref().and_then(|path| {
            match Reader::open_readfile(path) {
                Ok(reader) => {
                    tracing::info!(path = %path.display(), "loaded MaxMind GeoLite2 database");
                    Some(reader)
                }
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "failed to load MaxMind database");
                    None
                }
            }
        });

        Self {
            ua_parser: UaParser::new(),
            geo_reader,
            config,
        }
    }

    /// Create an enricher with a pre-loaded geo database.
    pub fn with_geo_reader(config: Arc<AnalyticsConfig>, geo_reader: Reader<Vec<u8>>) -> Self {
        Self {
            ua_parser: UaParser::new(),
            geo_reader: Some(geo_reader),
            config,
        }
    }

    /// Enrich an event with server-side context.
    pub fn enrich(
        &self,
        event: &mut IngestEvent,
        client_ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> EnrichmentResult {
        let mut result = EnrichmentResult::default();

        // IP truncation
        if let Some(ip) = client_ip {
            result.ip_truncated = truncate_ip(ip);
        }

        // User agent parsing
        if let Some(ua) = user_agent {
            if let Some(parsed) = self.ua_parser.parse(ua) {
                result.browser = Some(parsed.name.to_string());
                result.os = Some(parsed.os.to_string());
                result.device_type = Some(device_type_from_category(parsed.category));
            }
        }

        // Geo lookup
        if let (Some(ref reader), Some(ip_str)) = (&self.geo_reader, client_ip) {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                if let Ok(country) = reader.lookup::<geoip2::Country>(ip) {
                    if let Some(c) = country.country {
                        result.country = c.iso_code.map(|s| s.to_string());
                    }
                }
            }
        }

        // Extract referrer host from context
        if let Some(ref page) = event.context.page {
            if let Some(ref referrer) = page.referrer {
                result.referrer_host = extract_host(referrer);
            }
            result.url = page.url.clone();
            result.path = page.path.clone();
        }

        // Extract UTM parameters
        if let Some(ref utm) = event.context.utm {
            result.utm_source = utm.source.clone();
            result.utm_medium = utm.medium.clone();
            result.utm_campaign = utm.campaign.clone();
        }

        // Also try extracting UTM from URL query params if not already set
        if result.utm_source.is_none() {
            if let Some(ref page) = event.context.page {
                if let Some(ref url) = page.url {
                    let extracted = extract_utm_from_url(url);
                    if result.utm_source.is_none() {
                        result.utm_source = extracted.source;
                    }
                }
            }
        }

        result
    }

    /// Strip denied properties from an event.
    pub fn strip_denied_properties(&self, event: &mut IngestEvent) {
        // Strip common PII from properties
        if let Some(props) = event.properties.as_object_mut() {
            for key in DENIED_PROPERTY_KEYS {
                props.remove(*key);
            }
        }
    }
}

/// Enrichment result with extracted/computed fields.
#[derive(Debug, Default)]
pub struct EnrichmentResult {
    /// Truncated IP address (IPv4 /24, IPv6 /48).
    pub ip_truncated: Option<String>,
    /// Browser name.
    pub browser: Option<String>,
    /// OS name.
    pub os: Option<String>,
    /// Device type (desktop, mobile, tablet, bot).
    pub device_type: Option<String>,
    /// Country ISO code.
    pub country: Option<String>,
    /// Referrer host.
    pub referrer_host: Option<String>,
    /// URL.
    pub url: Option<String>,
    /// Path.
    pub path: Option<String>,
    /// UTM source.
    pub utm_source: Option<String>,
    /// UTM medium.
    pub utm_medium: Option<String>,
    /// UTM campaign.
    pub utm_campaign: Option<String>,
}

/// Truncate an IP address for privacy.
///
/// IPv4: Truncate to /24 (zero the last octet).
/// IPv6: Truncate to /48 (zero the last 80 bits).
pub fn truncate_ip(ip: &str) -> Option<String> {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            let octets = v4.octets();
            Some(format!("{}.{}.{}.0", octets[0], octets[1], octets[2]))
        }
        Ok(IpAddr::V6(v6)) => {
            let segments = v6.segments();
            Some(format!(
                "{:x}:{:x}:{:x}::0",
                segments[0], segments[1], segments[2]
            ))
        }
        Err(_) => None,
    }
}

/// Extract host from a URL.
fn extract_host(url: &str) -> Option<String> {
    Url::parse(url).ok()?.host_str().map(|s| s.to_string())
}

/// Extract UTM parameters from a URL.
fn extract_utm_from_url(url: &str) -> UtmContext {
    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return UtmContext::default(),
    };

    let mut utm = UtmContext::default();
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "utm_source" => utm.source = Some(value.into_owned()),
            "utm_medium" => utm.medium = Some(value.into_owned()),
            "utm_campaign" => utm.campaign = Some(value.into_owned()),
            "utm_term" => utm.term = Some(value.into_owned()),
            "utm_content" => utm.content = Some(value.into_owned()),
            _ => {}
        }
    }
    utm
}

/// Convert woothee category to device type.
fn device_type_from_category(category: &str) -> String {
    match category {
        "pc" => "desktop".to_string(),
        "smartphone" => "mobile".to_string(),
        "mobilephone" => "mobile".to_string(),
        "tablet" => "tablet".to_string(),
        "crawler" => "bot".to_string(),
        _ => "unknown".to_string(),
    }
}

impl Default for UtmContext {
    fn default() -> Self {
        Self {
            source: None,
            medium: None,
            campaign: None,
            term: None,
            content: None,
        }
    }
}

/// Property keys that should be stripped (common PII).
const DENIED_PROPERTY_KEYS: &[&str] = &[
    "password",
    "secret",
    "token",
    "api_key",
    "apiKey",
    "credit_card",
    "creditCard",
    "ssn",
    "social_security",
    "socialSecurity",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_ipv4() {
        assert_eq!(truncate_ip("192.168.1.100"), Some("192.168.1.0".to_string()));
        assert_eq!(truncate_ip("10.0.0.255"), Some("10.0.0.0".to_string()));
    }

    #[test]
    fn test_truncate_ipv6() {
        assert_eq!(
            truncate_ip("2001:db8:85a3::8a2e:370:7334"),
            Some("2001:db8:85a3::0".to_string())
        );
    }

    #[test]
    fn test_truncate_invalid() {
        assert_eq!(truncate_ip("not-an-ip"), None);
    }

    #[test]
    fn test_extract_host() {
        assert_eq!(
            extract_host("https://example.com/path?query=1"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host("https://www.google.com"),
            Some("www.google.com".to_string())
        );
        assert_eq!(extract_host("not-a-url"), None);
    }

    #[test]
    fn test_extract_utm() {
        let utm = extract_utm_from_url(
            "https://example.com?utm_source=google&utm_medium=cpc&utm_campaign=summer",
        );
        assert_eq!(utm.source, Some("google".to_string()));
        assert_eq!(utm.medium, Some("cpc".to_string()));
        assert_eq!(utm.campaign, Some("summer".to_string()));
    }
}
