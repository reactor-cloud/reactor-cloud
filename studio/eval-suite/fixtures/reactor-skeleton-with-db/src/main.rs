use std::net::SocketAddr;

mod jobs;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql://localhost/test".to_string());
    println!("Using database: {}", db_url);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Starting server on {}", addr);
}
