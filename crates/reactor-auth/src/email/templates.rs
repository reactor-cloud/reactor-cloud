//! Email templates.

/// Email template generator.
pub struct EmailTemplate;

impl EmailTemplate {
    /// Generate an invitation email.
    pub fn invitation(
        org_name: &str,
        role_name: &str,
        invite_link: &str,
        expires_hours: u32,
    ) -> (String, String, String) {
        let subject = format!("You've been invited to join {} on Reactor", org_name);

        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Invitation</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h1 style="color: #1a1a1a; margin-bottom: 24px;">You're invited!</h1>
    
    <p>You've been invited to join <strong>{org_name}</strong> as a <strong>{role_name}</strong> on Reactor.</p>
    
    <p style="margin: 32px 0;">
        <a href="{invite_link}" 
           style="display: inline-block; background-color: #0066FF; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">
            Accept Invitation
        </a>
    </p>
    
    <p style="color: #666; font-size: 14px;">
        This invitation will expire in {expires_hours} hours.
    </p>
    
    <p style="color: #666; font-size: 14px;">
        If you didn't expect this invitation, you can safely ignore this email.
    </p>
    
    <hr style="border: none; border-top: 1px solid #eee; margin: 32px 0;">
    
    <p style="color: #999; font-size: 12px;">
        Reactor.cloud — Build faster with AI-first backends
    </p>
</body>
</html>"#,
            org_name = org_name,
            role_name = role_name,
            invite_link = invite_link,
            expires_hours = expires_hours
        );

        let text = format!(
            r#"You're invited!

You've been invited to join {org_name} as a {role_name} on Reactor.

Accept the invitation by visiting:
{invite_link}

This invitation will expire in {expires_hours} hours.

If you didn't expect this invitation, you can safely ignore this email.

--
Reactor.cloud — Build faster with AI-first backends"#,
            org_name = org_name,
            role_name = role_name,
            invite_link = invite_link,
            expires_hours = expires_hours
        );

        (subject, html, text)
    }

    /// Generate a password reset email.
    pub fn password_reset(reset_link: &str, expires_minutes: u32) -> (String, String, String) {
        let subject = "Reset your Reactor password".to_string();

        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Password Reset</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h1 style="color: #1a1a1a; margin-bottom: 24px;">Reset your password</h1>
    
    <p>We received a request to reset your password. Click the button below to choose a new password.</p>
    
    <p style="margin: 32px 0;">
        <a href="{reset_link}" 
           style="display: inline-block; background-color: #0066FF; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">
            Reset Password
        </a>
    </p>
    
    <p style="color: #666; font-size: 14px;">
        This link will expire in {expires_minutes} minutes.
    </p>
    
    <p style="color: #666; font-size: 14px;">
        If you didn't request a password reset, you can safely ignore this email.
    </p>
    
    <hr style="border: none; border-top: 1px solid #eee; margin: 32px 0;">
    
    <p style="color: #999; font-size: 12px;">
        Reactor.cloud — Build faster with AI-first backends
    </p>
</body>
</html>"#,
            reset_link = reset_link,
            expires_minutes = expires_minutes
        );

        let text = format!(
            r#"Reset your password

We received a request to reset your password. Visit the link below to choose a new password:

{reset_link}

This link will expire in {expires_minutes} minutes.

If you didn't request a password reset, you can safely ignore this email.

--
Reactor.cloud — Build faster with AI-first backends"#,
            reset_link = reset_link,
            expires_minutes = expires_minutes
        );

        (subject, html, text)
    }

    /// Generate an email verification email.
    pub fn email_verification(verify_link: &str, expires_hours: u32) -> (String, String, String) {
        let subject = "Verify your email address".to_string();

        let html = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Email Verification</title>
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h1 style="color: #1a1a1a; margin-bottom: 24px;">Verify your email</h1>
    
    <p>Thanks for signing up! Please verify your email address by clicking the button below.</p>
    
    <p style="margin: 32px 0;">
        <a href="{verify_link}" 
           style="display: inline-block; background-color: #0066FF; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">
            Verify Email
        </a>
    </p>
    
    <p style="color: #666; font-size: 14px;">
        This link will expire in {expires_hours} hours.
    </p>
    
    <hr style="border: none; border-top: 1px solid #eee; margin: 32px 0;">
    
    <p style="color: #999; font-size: 12px;">
        Reactor.cloud — Build faster with AI-first backends
    </p>
</body>
</html>"#,
            verify_link = verify_link,
            expires_hours = expires_hours
        );

        let text = format!(
            r#"Verify your email

Thanks for signing up! Please verify your email address by visiting:

{verify_link}

This link will expire in {expires_hours} hours.

--
Reactor.cloud — Build faster with AI-first backends"#,
            verify_link = verify_link,
            expires_hours = expires_hours
        );

        (subject, html, text)
    }
}
