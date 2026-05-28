//! CLI command implementations.

pub mod ai;
pub mod auth;
pub mod build;
pub mod cloud;
pub mod connect;
pub mod cloud_domains;
pub mod context;
pub mod data;
pub mod deploy;
pub mod doctor;
pub mod functions;
pub mod init;
pub mod inspect;
pub mod jobs;
pub mod login;
pub mod logout;
pub mod logs;
pub mod migrate;
pub mod project;
pub mod sites;
pub mod types;
pub mod vault;
pub mod version;
pub mod whoami;

#[cfg(feature = "dev")]
pub mod dev;
#[cfg(feature = "dev")]
pub mod down;
#[cfg(feature = "dev")]
pub mod status;
#[cfg(feature = "dev")]
pub mod up;
