//! Proxy trust middleware for handling X-Forwarded-* headers.
//!
//! This module provides middleware that validates and extracts forwarded headers
//! only from trusted proxy IP ranges (e.g., Fly.io WireGuard mesh).

use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::Response,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use tracing::debug;

/// Trusted proxy ranges configuration.
#[derive(Debug, Clone)]
pub struct TrustedProxies {
    /// List of trusted IP ranges (CIDR notation internally).
    ranges: Vec<IpRange>,
}

/// An IP range (CIDR block).
#[derive(Debug, Clone)]
struct IpRange {
    network: IpAddr,
    prefix_len: u8,
}

impl IpRange {
    fn contains(&self, ip: &IpAddr) -> bool {
        match (self.network, ip) {
            (IpAddr::V4(net), IpAddr::V4(addr)) => {
                let net_bits = u32::from(net);
                let addr_bits = u32::from(*addr);
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    !0u32 << (32 - self.prefix_len)
                };
                (net_bits & mask) == (addr_bits & mask)
            }
            (IpAddr::V6(net), IpAddr::V6(addr)) => {
                let net_bits = u128::from(net);
                let addr_bits = u128::from(*addr);
                let mask = if self.prefix_len == 0 {
                    0
                } else {
                    !0u128 << (128 - self.prefix_len)
                };
                (net_bits & mask) == (addr_bits & mask)
            }
            _ => false,
        }
    }
}

impl TrustedProxies {
    /// Create a new trusted proxies configuration.
    pub fn new() -> Self {
        Self { ranges: Vec::new() }
    }

    /// Create with default Fly.io WireGuard ranges.
    ///
    /// Fly.io uses these internal IP ranges for WireGuard mesh:
    /// - fdaa::/16 (IPv6 private)
    /// - 172.19.0.0/16 (IPv4 private for internal services)
    pub fn fly_defaults() -> Self {
        let mut proxies = Self::new();
        
        // Fly.io WireGuard IPv6 range
        proxies.add_range("fdaa::", 16);
        
        // Fly.io internal IPv4 range
        proxies.add_range("172.19.0.0", 16);
        
        // Also trust localhost for development
        proxies.add_range("127.0.0.1", 32);
        proxies.add_range("::1", 128);
        
        proxies
    }

    /// Add a CIDR range to trusted proxies.
    pub fn add_range(&mut self, network: &str, prefix_len: u8) {
        if let Ok(ip) = network.parse::<IpAddr>() {
            self.ranges.push(IpRange { network: ip, prefix_len });
        }
    }

    /// Check if an IP is trusted.
    pub fn is_trusted(&self, ip: &IpAddr) -> bool {
        self.ranges.iter().any(|range| range.contains(ip))
    }
}

impl Default for TrustedProxies {
    fn default() -> Self {
        Self::fly_defaults()
    }
}

/// Extension containing the real client IP after proxy processing.
#[derive(Debug, Clone)]
pub struct RealClientIp(pub Option<IpAddr>);

/// Middleware that extracts the real client IP from X-Forwarded-For headers
/// only if the request comes from a trusted proxy.
pub async fn trusted_proxy_middleware(
    trusted: Arc<TrustedProxies>,
    request: Request,
    next: Next,
) -> Response {
    let mut request = request;
    
    // Get the immediate connection IP
    let connect_info = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip());

    let real_ip = if let Some(peer_ip) = connect_info {
        if trusted.is_trusted(&peer_ip) {
            // Trusted proxy - extract X-Forwarded-For
            extract_forwarded_ip(&request).or(Some(peer_ip))
        } else {
            // Untrusted source - use connection IP
            Some(peer_ip)
        }
    } else {
        // No connection info available
        extract_forwarded_ip(&request)
    };

    debug!("Real client IP: {:?}", real_ip);

    // Insert the real IP into extensions
    request.extensions_mut().insert(RealClientIp(real_ip));

    next.run(request).await
}

/// Extract IP from X-Forwarded-For header.
fn extract_forwarded_ip(request: &Request) -> Option<IpAddr> {
    // Check X-Forwarded-For first (standard)
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(value) = xff.to_str() {
            // X-Forwarded-For can contain multiple IPs: "client, proxy1, proxy2"
            // The first one is the original client
            if let Some(first_ip) = value.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse() {
                    return Some(ip);
                }
            }
        }
    }

    // Check X-Real-IP (nginx style)
    if let Some(xri) = request.headers().get("x-real-ip") {
        if let Ok(value) = xri.to_str() {
            if let Ok(ip) = value.trim().parse() {
                return Some(ip);
            }
        }
    }

    // Check Forwarded header (RFC 7239)
    if let Some(forwarded) = request.headers().get("forwarded") {
        if let Ok(value) = forwarded.to_str() {
            // Parse "for=" directive
            for part in value.split(';') {
                let part = part.trim();
                if part.to_lowercase().starts_with("for=") {
                    let ip_str = &part[4..].trim_matches('"');
                    // Handle IPv6 in brackets
                    let ip_str = ip_str.trim_start_matches('[').trim_end_matches(']');
                    // Remove port if present
                    let ip_str = ip_str.split(':').next().unwrap_or(ip_str);
                    if let Ok(ip) = ip_str.parse() {
                        return Some(ip);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_range_contains_v4() {
        let range = IpRange {
            network: "192.168.0.0".parse().unwrap(),
            prefix_len: 24,
        };

        assert!(range.contains(&"192.168.0.1".parse().unwrap()));
        assert!(range.contains(&"192.168.0.255".parse().unwrap()));
        assert!(!range.contains(&"192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_ip_range_contains_v6() {
        let range = IpRange {
            network: "fdaa::".parse().unwrap(),
            prefix_len: 16,
        };

        assert!(range.contains(&"fdaa::1".parse().unwrap()));
        assert!(range.contains(&"fdaa:1234::1".parse().unwrap()));
        assert!(!range.contains(&"fdab::1".parse().unwrap()));
    }

    #[test]
    fn test_fly_defaults() {
        let proxies = TrustedProxies::fly_defaults();

        // Fly WireGuard IPs should be trusted
        assert!(proxies.is_trusted(&"fdaa::1".parse().unwrap()));
        assert!(proxies.is_trusted(&"172.19.0.1".parse().unwrap()));
        
        // Localhost should be trusted
        assert!(proxies.is_trusted(&"127.0.0.1".parse().unwrap()));
        
        // Public IPs should not be trusted
        assert!(!proxies.is_trusted(&"8.8.8.8".parse().unwrap()));
    }
}
