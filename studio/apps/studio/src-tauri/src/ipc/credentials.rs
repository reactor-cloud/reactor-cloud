use serde::{Deserialize, Serialize};
use keyring::Entry;

const SERVICE_NAME: &str = "reactor-studio";

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("Keyring error: {0}")]
    Keyring(String),
    #[error("Credential not found")]
    NotFound,
}

impl Serialize for CredentialError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Credential info returned to frontend (without the secret value)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialInfo {
    pub key: String,
    pub is_set: bool,
}

#[tauri::command]
pub async fn credential_set(key: String, value: String) -> Result<(), CredentialError> {
    let entry = Entry::new(SERVICE_NAME, &key)
        .map_err(|e| CredentialError::Keyring(e.to_string()))?;
    
    entry.set_password(&value)
        .map_err(|e| CredentialError::Keyring(e.to_string()))?;
    
    Ok(())
}

#[tauri::command]
pub async fn credential_get(key: String) -> Result<String, CredentialError> {
    let entry = Entry::new(SERVICE_NAME, &key)
        .map_err(|e| CredentialError::Keyring(e.to_string()))?;
    
    match entry.get_password() {
        Ok(pwd) => Ok(pwd),
        Err(keyring::Error::NoEntry) => Err(CredentialError::NotFound),
        Err(e) => Err(CredentialError::Keyring(e.to_string())),
    }
}

#[tauri::command]
pub async fn credential_delete(key: String) -> Result<(), CredentialError> {
    let entry = Entry::new(SERVICE_NAME, &key)
        .map_err(|e| CredentialError::Keyring(e.to_string()))?;
    
    match entry.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(CredentialError::Keyring(e.to_string())),
    }
}

#[tauri::command]
pub async fn credential_check(key: String) -> Result<CredentialInfo, CredentialError> {
    let entry = Entry::new(SERVICE_NAME, &key)
        .map_err(|e| CredentialError::Keyring(e.to_string()))?;
    
    let is_set = entry.get_password().is_ok();
    
    Ok(CredentialInfo {
        key,
        is_set,
    })
}

/// List of known credential keys
#[tauri::command]
pub async fn credential_list() -> Result<Vec<CredentialInfo>, CredentialError> {
    let keys = vec!["openrouter_api_key", "openai_api_key"];
    
    let mut infos = Vec::new();
    for key in keys {
        let entry = Entry::new(SERVICE_NAME, key)
            .map_err(|e| CredentialError::Keyring(e.to_string()))?;
        
        let is_set = entry.get_password().is_ok();
        infos.push(CredentialInfo {
            key: key.to_string(),
            is_set,
        });
    }
    
    Ok(infos)
}
