//! Login command implementation.
//!
//! Supports two login modes:
//! - **Token mode** (default for legacy compatibility): Paste a token directly
//! - **Browser mode** (`--browser`): PKCE OAuth flow that opens a browser
//!
//! The browser mode is recommended for operators as it provides:
//! - Proper session management with refresh tokens
//! - Scope-limited access
//! - MFA step-up capability for sensitive operations

use crate::cli::{Cli, LoginArgs};
use crate::context::{AuthConfig, GlobalConfig, TokenFile, TokenStorage};
use crate::error::{CliError, CliResult};
use crate::output::Output;
use reactor_client::{Client, ClientConfig};
use std::io::{self, BufRead};

pub async fn run(cli: &Cli, args: &LoginArgs, output: &Output) -> CliResult<()> {
    let mut config = GlobalConfig::load()?;

    // Determine context name
    let context_name = args
        .context
        .as_deref()
        .or(cli.context.as_deref())
        .or(config.default.as_deref())
        .ok_or_else(|| CliError::Config("no context specified and no default set".into()))?
        .to_string();

    // Check if context exists
    let ctx = config
        .get_context(&context_name)
        .ok_or_else(|| CliError::ContextNotFound(context_name.clone()))?
        .clone();

    // Check if browser login is requested
    if args.browser {
        return run_pkce_login(cli, args, &ctx, &context_name, &mut config, output).await;
    }

    // Legacy token-paste flow
    run_token_login(cli, args, &ctx, &context_name, &mut config, output).await
}

/// Run the legacy token-paste login flow.
async fn run_token_login(
    cli: &Cli,
    args: &LoginArgs,
    ctx: &crate::context::ContextConfig,
    context_name: &str,
    config: &mut GlobalConfig,
    output: &Output,
) -> CliResult<()> {
    // Get token from args, flag, or stdin
    let token = if let Some(t) = &args.token {
        t.clone()
    } else if let Some(t) = &cli.token {
        t.clone()
    } else {
        // Read from stdin
        output.info("Enter authentication token:");
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let token = line.trim().to_string();
        if token.is_empty() {
            return Err(CliError::AuthRequired);
        }
        token
    };

    // Validate token by calling the server
    output.info(&format!("Validating token against {}...", ctx.endpoint));
    let client_config = ClientConfig::new(ctx.endpoint.parse()?)
        .with_token(&token);
    let client = Client::new(client_config)?;

    // Try to get version info to validate the token
    match client.version().await {
        Ok(version_info) => {
            output.info(&format!(
                "Connected to Reactor server v{}",
                version_info.reactor_server
            ));
        }
        Err(e) => {
            return Err(CliError::AuthFailed(format!(
                "Failed to validate token: {}",
                e
            )));
        }
    }

    // Store token
    #[cfg(feature = "keyring")]
    {
        crate::context::store_token_keychain(context_name, &token)?;

        // Update context config to use keychain
        let mut updated_ctx = ctx.clone();
        updated_ctx.auth = AuthConfig::Keychain {
            service: "reactor".to_string(),
            account: context_name.to_string(),
        };
        config.set_context(context_name.to_string(), updated_ctx);
        config.save()?;
    }

    #[cfg(not(feature = "keyring"))]
    {
        // Without keyring, suggest using environment variable
        let env_name = format!(
            "REACTOR_{}_TOKEN",
            context_name.to_uppercase().replace('-', "_")
        );
        output.warning(&format!(
            "Keyring not available. Set {} environment variable to persist the token.",
            env_name
        ));
    }

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "context": context_name,
            "authenticated": true,
            "mode": "token"
        }))?;
    } else {
        output.success_message(&format!(
            "Logged in to context '{}'.",
            context_name
        ))?;
    }

    Ok(())
}

/// Run the PKCE browser login flow.
///
/// This flow:
/// 1. Generates a PKCE code verifier and challenge
/// 2. Starts a localhost HTTP server to receive the callback
/// 3. Opens the browser to the authorization URL
/// 4. Waits for the callback with the authorization code
/// 5. Exchanges the code for access and refresh tokens
/// 6. Stores the tokens in the keychain
#[cfg(feature = "browser-login")]
async fn run_pkce_login(
    _cli: &Cli,
    args: &LoginArgs,
    ctx: &crate::context::ContextConfig,
    context_name: &str,
    config: &mut GlobalConfig,
    output: &Output,
) -> CliResult<()> {
    use base64::Engine;
    use rand::Rng;
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::net::TcpListener;
    use tokio::sync::oneshot;

    // For --no-browser, use device code flow instead
    if args.no_browser {
        return run_device_code_login(ctx, context_name, args.file_storage, config, output).await;
    }

    output.info(&format!("Starting browser login for {}...", ctx.endpoint));

    // 1. Generate PKCE code verifier (43-128 chars from [A-Za-z0-9-._~])
    let code_verifier: String = {
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| {
                const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
                CHARSET[rng.gen_range(0..CHARSET.len())] as char
            })
            .collect()
    };

    // 2. Compute S256 challenge = base64url(sha256(verifier))
    let code_challenge = {
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
    };

    // Generate random state for CSRF protection
    let state: String = {
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| {
                const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
                CHARSET[rng.gen_range(0..CHARSET.len())] as char
            })
            .collect()
    };

    // 3. Find an available port and start localhost HTTP server
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| CliError::Internal(format!("Failed to bind callback server: {}", e)))?;
    let port = listener
        .local_addr()
        .map_err(|e| CliError::Internal(format!("Failed to get port: {}", e)))?
        .port();

    let redirect_uri = format!("http://localhost:{}/callback", port);

    // 4. Build authorize URL
    let scopes = if args.bootstrap {
        "ops:* cloud:* platform:bootstrap"
    } else {
        "ops:* cloud:*"
    };

    let auth_url = format!(
        "{}/auth/v1/authorize?response_type=code&client_id=reactor-cli&redirect_uri={}&code_challenge={}&code_challenge_method=S256&state={}&scope={}",
        ctx.endpoint,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&code_challenge),
        urlencoding::encode(&state),
        urlencoding::encode(scopes),
    );

    // Channel to receive the authorization code
    let (tx, rx) = oneshot::channel::<Result<String, String>>();
    let expected_state = state.clone();

    // Start the callback server in background
    let callback_handle = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        
        let listener = tokio::net::TcpListener::from_std(listener)
            .expect("Failed to convert listener");
        
        // Wait for a single connection
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            listener.accept(),
        );
        
        let result = match timeout.await {
            Ok(Ok((mut stream, _))) => {
                // Read HTTP request
                let (reader, mut writer) = stream.split();
                let mut reader = BufReader::new(reader);
                let mut request_line = String::new();
                
                if reader.read_line(&mut request_line).await.is_err() {
                    Err("Failed to read request".to_string())
                } else {
                    // Parse GET /callback?code=...&state=...
                    let result = if let Some(query_start) = request_line.find('?') {
                        let path_end = request_line.find(" HTTP/").unwrap_or(request_line.len());
                        let query = &request_line[query_start + 1..path_end];
                        let params: HashMap<String, String> = query
                            .split('&')
                            .filter_map(|pair| {
                                let mut parts = pair.splitn(2, '=');
                                Some((
                                    parts.next()?.to_string(),
                                    urlencoding::decode(parts.next().unwrap_or(""))
                                        .unwrap_or_default()
                                        .to_string(),
                                ))
                            })
                            .collect();
                        
                        if let Some(error) = params.get("error") {
                            let desc = params.get("error_description").cloned().unwrap_or_default();
                            Err(format!("{}: {}", error, desc))
                        } else if let (Some(code), Some(recv_state)) = (params.get("code"), params.get("state")) {
                            if recv_state == &expected_state {
                                Ok(code.clone())
                            } else {
                                Err("State mismatch - possible CSRF attack".to_string())
                            }
                        } else {
                            Err("Missing code or state in callback".to_string())
                        }
                    } else {
                        Err("Invalid callback URL".to_string())
                    };
                    
                    // Send HTTP response with professional dark-mode styling
                    let (status, body) = match &result {
                        Ok(_) => ("200 OK", r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Reactor - Login Successful</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            background: linear-gradient(135deg, #0a0a0a 0%, #1a1a2e 100%);
            color: #e4e4e7;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .container {
            text-align: center;
            padding: 3rem 2rem;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
            max-width: 420px;
        }
        .logo {
            font-size: 1.5rem;
            font-weight: 700;
            letter-spacing: -0.02em;
            margin-bottom: 2rem;
            background: linear-gradient(135deg, #60a5fa 0%, #a78bfa 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }
        .icon {
            width: 64px;
            height: 64px;
            background: linear-gradient(135deg, #22c55e 0%, #16a34a 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 1.5rem;
        }
        .icon svg { width: 32px; height: 32px; color: white; }
        h1 { font-size: 1.5rem; font-weight: 600; margin-bottom: 0.75rem; }
        p { color: #a1a1aa; line-height: 1.6; }
    </style>
</head>
<body>
    <div class="container">
        <div class="logo">Reactor</div>
        <div class="icon">
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
            </svg>
        </div>
        <h1>Login Successful</h1>
        <p>You can close this window and return to the CLI.</p>
    </div>
</body>
</html>"#.to_string()),
                        Err(e) => ("400 Bad Request", format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>Reactor - Login Failed</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            background: linear-gradient(135deg, #0a0a0a 0%, #1a1a2e 100%);
            color: #e4e4e7;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
        }}
        .container {{
            text-align: center;
            padding: 3rem 2rem;
            background: rgba(255, 255, 255, 0.03);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 16px;
            backdrop-filter: blur(10px);
            max-width: 420px;
        }}
        .logo {{
            font-size: 1.5rem;
            font-weight: 700;
            letter-spacing: -0.02em;
            margin-bottom: 2rem;
            background: linear-gradient(135deg, #60a5fa 0%, #a78bfa 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}
        .icon {{
            width: 64px;
            height: 64px;
            background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
            border-radius: 50%;
            display: flex;
            align-items: center;
            justify-content: center;
            margin: 0 auto 1.5rem;
        }}
        .icon svg {{ width: 32px; height: 32px; color: white; }}
        h1 {{ font-size: 1.5rem; font-weight: 600; margin-bottom: 0.75rem; }}
        p {{ color: #a1a1aa; line-height: 1.6; }}
        .error {{ color: #fca5a5; font-family: monospace; font-size: 0.875rem; margin-top: 1rem; padding: 1rem; background: rgba(239, 68, 68, 0.1); border-radius: 8px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="logo">Reactor</div>
        <div class="icon">
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
            </svg>
        </div>
        <h1>Login Failed</h1>
        <p>An error occurred during authentication.</p>
        <div class="error">{}</div>
    </div>
</body>
</html>"#, e)),
                    };
                    
                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );
                    let _ = writer.write_all(response.as_bytes()).await;
                    
                    result
                }
            }
            Ok(Err(e)) => Err(format!("Accept error: {}", e)),
            Err(_) => Err("Timeout waiting for callback".to_string()),
        };
        
        let _ = tx.send(result);
    });

    // 5. Open browser
    output.info("Opening browser for authentication...");
    output.info(&format!("If browser doesn't open, visit: {}", auth_url));

    if webbrowser::open(&auth_url).is_err() {
        output.warning("Failed to open browser automatically.");
        output.info("Please manually open this URL in your browser:");
        output.info(&auth_url);
    }

    // 6. Wait for callback
    output.info("Waiting for authorization...");

    let code = match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
        Ok(Ok(Ok(code))) => code,
        Ok(Ok(Err(e))) => {
            callback_handle.abort();
            return Err(CliError::AuthFailed(e));
        }
        Ok(Err(_)) => {
            callback_handle.abort();
            return Err(CliError::AuthFailed("Callback channel closed".to_string()));
        }
        Err(_) => {
            callback_handle.abort();
            return Err(CliError::AuthFailed("Authorization timed out".to_string()));
        }
    };

    callback_handle.abort();

    // 7. Exchange code for tokens
    output.info("Exchanging authorization code for tokens...");

    let client = reqwest::Client::new();
    let token_response = client
        .post(format!("{}/auth/v1/token", ctx.endpoint))
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", "reactor-cli"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &code_verifier),
        ])
        .send()
        .await
        .map_err(|e| CliError::AuthFailed(format!("Token exchange failed: {}", e)))?;

    if !token_response.status().is_success() {
        let error_text = token_response.text().await.unwrap_or_default();
        return Err(CliError::AuthFailed(format!(
            "Token exchange failed: {}",
            error_text
        )));
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
        #[serde(default)]
        scope: String,
    }

    let tokens: TokenResponse = token_response
        .json()
        .await
        .map_err(|e| CliError::AuthFailed(format!("Failed to parse token response: {}", e)))?;

    let expires_at = chrono::Utc::now().timestamp() + tokens.expires_in;
    let scopes: Vec<String> = tokens.scope.split_whitespace().map(String::from).collect();

    // 8. Store tokens
    let access_account = format!("{}_access", context_name);
    let refresh_account = format!("{}_refresh", context_name);
    let storage = if args.file_storage { TokenStorage::File } else { TokenStorage::Keychain };

    if args.file_storage {
        // Store in file
        tracing::debug!(access_account = %access_account, "Storing tokens in file storage");
        
        let mut token_file = TokenFile::load()?;
        token_file.set(&access_account, &tokens.access_token);
        token_file.set(&refresh_account, &tokens.refresh_token);
        token_file.save()?;
        
        tracing::debug!("Tokens stored in file successfully");
    } else {
        // Store in keychain
        #[cfg(feature = "keyring")]
        {
            tracing::debug!(access_account = %access_account, "Creating keyring entry for access token");

            // Store access token
            let keyring_access = keyring::Entry::new("reactor", &access_account)
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to create keyring entry for access token");
                    CliError::Keychain(e.to_string())
                })?;
            
            keyring_access
                .set_password(&tokens.access_token)
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to store access token in keyring");
                    CliError::Keychain(e.to_string())
                })?;
            
            tracing::debug!("Access token stored successfully");

            // Store refresh token
            let keyring_refresh = keyring::Entry::new("reactor", &refresh_account)
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to create keyring entry for refresh token");
                    CliError::Keychain(e.to_string())
                })?;
            keyring_refresh
                .set_password(&tokens.refresh_token)
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to store refresh token in keyring");
                    CliError::Keychain(e.to_string())
                })?;

            tracing::debug!("Refresh token stored successfully");
        }

        #[cfg(not(feature = "keyring"))]
        {
            return Err(CliError::Internal(
                "Keychain storage requires keyring feature. Use --file-storage instead.".to_string(),
            ));
        }
    }

    // Update context config
    let mut updated_ctx = ctx.clone();
    updated_ctx.auth = AuthConfig::Session {
        storage,
        service: "reactor".to_string(),
        access_account: access_account.clone(),
        refresh_account: refresh_account.clone(),
        expires_at,
        scopes: scopes.clone(),
    };
    config.set_context(context_name.to_string(), updated_ctx);
    config.save()?;
    
    tracing::debug!(access_account = %access_account, refresh_account = %refresh_account, "Session config saved");

    if output.format().is_json() {
        output.success(&serde_json::json!({
            "context": context_name,
            "authenticated": true,
            "mode": "pkce",
            "expires_at": expires_at,
            "scopes": scopes
        }))?;
    } else {
        output.success_message(&format!(
            "Logged in to context '{}' with scopes: {}",
            context_name,
            scopes.join(", ")
        ))?;
    }

    Ok(())
}

#[cfg(not(feature = "browser-login"))]
async fn run_pkce_login(
    _cli: &Cli,
    _args: &LoginArgs,
    _ctx: &crate::context::ContextConfig,
    _context_name: &str,
    _config: &mut GlobalConfig,
    output: &Output,
) -> CliResult<()> {
    output.error("Browser login requires the 'browser-login' feature. Use token login instead.");
    Err(CliError::Internal(
        "Browser login not available - compile with browser-login feature".to_string(),
    ))
}

/// Run the device code login flow for environments without a browser.
async fn run_device_code_login(
    ctx: &crate::context::ContextConfig,
    context_name: &str,
    file_storage: bool,
    config: &mut GlobalConfig,
    output: &Output,
) -> CliResult<()> {
    output.info(&format!("Starting device code login for {}...", ctx.endpoint));

    // 1. Request device code
    let client = reqwest::Client::new();
    let device_response = client
        .post(format!("{}/auth/v1/device/code", ctx.endpoint))
        .form(&[
            ("client_id", "reactor-cli"),
            ("scope", "ops:* cloud:*"),
        ])
        .send()
        .await
        .map_err(|e| CliError::AuthFailed(format!("Device code request failed: {}", e)))?;

    if !device_response.status().is_success() {
        let error_text = device_response.text().await.unwrap_or_default();
        return Err(CliError::AuthFailed(format!(
            "Device code request failed: {}",
            error_text
        )));
    }

    #[derive(serde::Deserialize)]
    struct DeviceCodeResponse {
        device_code: String,
        user_code: String,
        verification_uri: String,
        #[serde(default = "default_interval")]
        interval: u64,
        expires_in: i64,
    }

    fn default_interval() -> u64 {
        5
    }

    let device: DeviceCodeResponse = device_response
        .json()
        .await
        .map_err(|e| CliError::AuthFailed(format!("Failed to parse device code response: {}", e)))?;

    // 2. Display instructions
    output.info("");
    output.info("To complete login:");
    output.info(&format!("  1. Visit: {}", device.verification_uri));
    output.info(&format!("  2. Enter code: {}", device.user_code));
    output.info("");
    output.info("Waiting for authorization...");

    // 3. Poll for tokens
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(device.expires_in as u64);

    loop {
        if std::time::Instant::now() > deadline {
            return Err(CliError::AuthFailed("Device authorization expired".to_string()));
        }

        tokio::time::sleep(std::time::Duration::from_secs(device.interval)).await;

        let token_response = client
            .post(format!("{}/auth/v1/token", ctx.endpoint))
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", "reactor-cli"),
                ("device_code", &device.device_code),
            ])
            .send()
            .await
            .map_err(|e| CliError::AuthFailed(format!("Token poll failed: {}", e)))?;

        if token_response.status().is_success() {
            #[derive(serde::Deserialize)]
            struct TokenResponse {
                access_token: String,
                refresh_token: String,
                expires_in: i64,
                #[serde(default)]
                scope: String,
            }

            let tokens: TokenResponse = token_response
                .json()
                .await
                .map_err(|e| CliError::AuthFailed(format!("Failed to parse tokens: {}", e)))?;

            let expires_at = chrono::Utc::now().timestamp() + tokens.expires_in;
            let scopes: Vec<String> = tokens.scope.split_whitespace().map(String::from).collect();

            let access_account = format!("{}_access", context_name);
            let refresh_account = format!("{}_refresh", context_name);
            let storage = if file_storage { TokenStorage::File } else { TokenStorage::Keychain };

            if file_storage {
                let mut token_file = TokenFile::load()?;
                token_file.set(&access_account, &tokens.access_token);
                token_file.set(&refresh_account, &tokens.refresh_token);
                token_file.save()?;
            } else {
                #[cfg(feature = "keyring")]
                {
                    let keyring_access = keyring::Entry::new("reactor", &access_account)
                        .map_err(|e| CliError::Keychain(e.to_string()))?;
                    keyring_access
                        .set_password(&tokens.access_token)
                        .map_err(|e| CliError::Keychain(e.to_string()))?;

                    let keyring_refresh = keyring::Entry::new("reactor", &refresh_account)
                        .map_err(|e| CliError::Keychain(e.to_string()))?;
                    keyring_refresh
                        .set_password(&tokens.refresh_token)
                        .map_err(|e| CliError::Keychain(e.to_string()))?;
                }

                #[cfg(not(feature = "keyring"))]
                {
                    return Err(CliError::Internal(
                        "Keychain storage requires keyring feature. Use --file-storage instead.".to_string(),
                    ));
                }
            }

            let mut updated_ctx = ctx.clone();
            updated_ctx.auth = AuthConfig::Session {
                storage,
                service: "reactor".to_string(),
                access_account,
                refresh_account,
                expires_at,
                scopes: scopes.clone(),
            };
            config.set_context(context_name.to_string(), updated_ctx);
            config.save()?;

            if output.format().is_json() {
                output.success(&serde_json::json!({
                    "context": context_name,
                    "authenticated": true,
                    "mode": "device_code",
                    "expires_at": expires_at,
                    "scopes": scopes
                }))?;
            } else {
                output.success_message(&format!(
                    "Logged in to context '{}' with scopes: {}",
                    context_name,
                    scopes.join(", ")
                ))?;
            }

            return Ok(());
        }

        // Check for pending or other errors
        #[derive(serde::Deserialize)]
        struct ErrorResponse {
            error: String,
            #[serde(default)]
            error_description: String,
        }

        if let Ok(error) = token_response.json::<ErrorResponse>().await {
            match error.error.as_str() {
                "authorization_pending" => {
                    // Keep polling
                    continue;
                }
                "slow_down" => {
                    // Increase interval
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
                _ => {
                    return Err(CliError::AuthFailed(format!(
                        "{}: {}",
                        error.error, error.error_description
                    )));
                }
            }
        }
    }
}
