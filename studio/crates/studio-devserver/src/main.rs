//! Studio Devserver - Development server for testing Reactor Studio
//!
//! Run with: cargo run --package studio-devserver

use studio_devserver::DevServer;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Get workspace path from args or use current directory
    let workspace_path = std::env::args()
        .nth(1)
        .map(PathBuf::from);
    
    println!("Starting Reactor Studio DevServer...");
    if let Some(ref path) = workspace_path {
        println!("Workspace: {:?}", path);
    }
    
    match DevServer::start(workspace_path.as_deref()).await {
        Ok(server) => {
            println!("DevServer running at {}", server.base_url());
            println!("Token: {}", server.discovery().token);
            
            // Keep the server running
            tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
            println!("\nShutting down...");
        }
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
            std::process::exit(1);
        }
    }
}
