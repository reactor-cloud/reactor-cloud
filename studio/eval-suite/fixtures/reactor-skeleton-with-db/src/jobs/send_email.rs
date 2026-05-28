use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SendEmailJob {
    pub to: String,
    pub subject: String,
    pub body: String,
}

impl SendEmailJob {
    pub async fn execute(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Sending email to {} with subject: {}", self.to, self.subject);
        Ok(())
    }
}
