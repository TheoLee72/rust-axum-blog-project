use super::sendmail::send_email;

/// Send email verification link to new users during registration
///
/// Creates a verification link with the token and sends it using the
/// Verification-email.html template.
pub async fn send_verification_email(
    to_email: &str,
    username: &str,
    token: &str,
    frontend_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "Email Verification";
    let template_path = "src/mail/templates/Verification-email.html";

    // Build verification link: https://example.com/auth/email/confirm/{token}
    let verification_link = format!("{}/auth/email/confirm/{}", frontend_url, token);

    let placeholders = vec![
        ("{{username}}".to_string(), username.to_string()),
        ("{{verification_link}}".to_string(), verification_link),
    ];

    send_email(to_email, subject, template_path, &placeholders).await
}

/// Send email verification link when user changes their email address
///
/// Uses a different template (Verification-newemail.html) to indicate
/// this is for an email change, not initial registration.
pub async fn send_verification_email_newemail(
    to_email: &str,
    username: &str,
    token: &str,
    frontend_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "Email Verification";
    let template_path = "src/mail/templates/Verification-newemail.html";
    let verification_link = format!("{}/auth/email/confirm/{}", frontend_url, token);
    let placeholders = vec![
        ("{{username}}".to_string(), username.to_string()),
        ("{{verification_link}}".to_string(), verification_link),
    ];

    send_email(to_email, subject, template_path, &placeholders).await
}

/// Send welcome email after successful email verification
///
/// Sent immediately after user verifies their email to confirm
/// successful registration and provide onboarding information.
pub async fn send_welcome_email(
    to_email: &str,
    username: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "Welcome to Application";
    let template_path = "src/mail/templates/Welcome-email.html";
    let placeholders = vec![("{{username}}".to_string(), username.to_string())];

    send_email(to_email, subject, template_path, &placeholders).await
}

/// Send password reset link for "Forgot Password" flow
///
/// The reset_link should be a complete URL including the reset token,
/// for me: https://example.com/auth/password/reset/{token}
pub async fn send_forgot_password_email(
    to_email: &str,
    reset_link: &str,
    username: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let subject = "Reset your Password";
    let template_path = "src/mail/templates/RestPassword-email.html";
    let placeholders = vec![
        ("{{username}}".to_string(), username.to_string()),
        ("{{reset_link}}".to_string(), reset_link.to_string()),
    ];

    send_email(to_email, subject, template_path, &placeholders).await
}
