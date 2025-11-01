use lettre::{
    Message, SmtpTransport, Transport,
    message::{SinglePart, header},
    transport::smtp::authentication::Credentials,
};
use std::{env, fs};

/// Send an HTML email using SMTP
///
/// Loads an HTML template file, replaces placeholders with actual values,
/// and sends the email via the configured SMTP server.
///
/// # Parameters
/// - `to_email`: Recipient's email address
/// - `subject`: Email subject line
/// - `template_path`: Path to HTML template file (e.g., "src/mail/templates/Welcome-email.html")
/// - `placeholders`: Key-value pairs to replace in template (e.g., {{username}} -> "John")
pub async fn send_email(
    to_email: &str,
    subject: &str,
    template_path: &str,
    placeholders: &[(String, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    // Load SMTP credentials from environment variables
    let smtp_username = env::var("SMTP_USERNAME")?;
    let smtp_password = env::var("SMTP_PASSWORD")?;
    let smtp_server = env::var("SMTP_SERVER")?; // e.g., "smtp.gmail.com"
    let smtp_port: u16 = env::var("SMTP_PORT")?.parse()?; // Usually 587 for STARTTLS

    // Read HTML template from file
    let mut html_template = fs::read_to_string(template_path)?;

    // Replace all placeholders with actual values
    // Example: {{username}} becomes "John", {{verification_link}} becomes "https://..."
    for (key, value) in placeholders {
        html_template = html_template.replace(key, value)
    }

    // Build the email message
    let email = Message::builder()
        .from(smtp_username.parse()?) // From address (usually same as SMTP username)
        .to(to_email.parse()?) // Recipient address
        .subject(subject) // Email subject
        .header(header::ContentType::TEXT_HTML)
        .singlepart(
            SinglePart::builder()
                .header(header::ContentType::TEXT_HTML)
                .body(html_template), // HTML content with placeholders replaced
        )?;

    // Configure SMTP transport with STARTTLS encryption
    let creds = Credentials::new(smtp_username.clone(), smtp_password.clone());
    let mailer = SmtpTransport::starttls_relay(&smtp_server)? // STARTTLS: starts unencrypted, upgrades to TLS
        .credentials(creds)
        .port(smtp_port)
        .build();

    // Send the email
    let result = mailer.send(&email);

    // Log result (in production, use proper logging instead of println)
    match result {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => println!("Failed to send email: {:?}", e),
    }

    Ok(())
}
