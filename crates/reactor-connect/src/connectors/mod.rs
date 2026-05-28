//! Native connector implementations.
//!
//! Each connector implements the `NativeConnector` trait.
//! These are first-party Rust connectors for popular services.

// M1.6 connectors
pub mod github;
pub mod linear;
pub mod slack;
pub mod stripe;

// M3.1 connector
pub mod salesforce;

pub use github::GitHubConnector;
pub use linear::LinearConnector;
pub use salesforce::SalesforceConnector;
pub use slack::SlackConnector;
pub use stripe::StripeConnector;
