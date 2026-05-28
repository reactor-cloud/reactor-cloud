//! Shutdown coordination.
//!
//! Provides a shutdown handle that can be used to coordinate graceful shutdown
//! across all capabilities and background tasks.

use tokio::signal;
use tokio::sync::watch;

/// Handle for coordinating shutdown.
#[derive(Clone)]
pub struct ShutdownHandle {
    /// Sender to signal shutdown.
    tx: watch::Sender<bool>,
    /// Receiver to check shutdown status.
    rx: watch::Receiver<bool>,
}

impl ShutdownHandle {
    /// Create a new shutdown handle.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }

    /// Get a receiver to listen for shutdown.
    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.rx.clone()
    }

    /// Signal shutdown to all listeners.
    pub fn shutdown(&self) {
        let _ = self.tx.send(true);
    }

    /// Check if shutdown has been signaled.
    pub fn is_shutdown(&self) -> bool {
        *self.rx.borrow()
    }
}

impl Default for ShutdownHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Wait for a shutdown signal (SIGTERM or SIGINT).
///
/// Returns when either signal is received.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("received Ctrl+C, starting graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("received SIGTERM, starting graceful shutdown");
        }
    }
}

/// Wait for shutdown signal and then signal the handle.
///
/// This is a convenience function that combines waiting for the OS signal
/// and triggering the internal shutdown handle.
pub async fn wait_and_signal(handle: &ShutdownHandle) {
    shutdown_signal().await;
    handle.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_handle() {
        let handle = ShutdownHandle::new();
        assert!(!handle.is_shutdown());

        handle.shutdown();
        assert!(handle.is_shutdown());
    }

    #[test]
    fn test_shutdown_receiver() {
        let handle = ShutdownHandle::new();
        let rx = handle.receiver();

        assert!(!*rx.borrow());
        handle.shutdown();
        assert!(*rx.borrow());
    }
}
