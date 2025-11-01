use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use crate::error::ErrorMessage;

/// Maximum allowed password length in characters
///
/// Why limit password length?
/// 1. DoS prevention: Extremely long passwords can cause excessive CPU usage during hashing
/// 2. Practical limit: 64 characters is more than sufficient for strong passwords
/// 3. Memory safety: Prevents memory exhaustion attacks
///
/// Note: This is characters, not bytes. Emoji and non-ASCII characters may use multiple bytes.
const MAX_PASSWORD_LENGTH: usize = 64;

/// Hash a password using Argon2id algorithm
///
/// **What is Argon2?**
/// Argon2 is a modern, secure password hashing algorithm that won the Password Hashing
/// Competition (PHC) in 2015. It's designed to be resistant to:
/// - Brute force attacks (slow and computationally expensive)
/// - GPU/ASIC attacks (memory-hard algorithm)
/// - Side-channel attacks
///
/// **Why Argon2id specifically?**
/// Argon2 has three variants:
/// - Argon2d: Fast, resistant to GPU attacks (vulnerable to side-channel attacks)
/// - Argon2i: Slower, resistant to side-channel attacks
/// - Argon2id: Hybrid - provides both protections (RECOMMENDED for password hashing)
///
/// **Hash Format:**
/// The output string follows the PHC string format:
/// ```
/// $argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
/// ```
/// - `argon2id`: Algorithm variant
/// - `v=19`: Version 1.3 of Argon2
/// - `m=19456`: Memory cost (19 MB)
/// - `t=2`: Time cost (2 iterations)
/// - `p=1`: Parallelism (1 thread)
/// - `<salt>`: Random salt (base64-encoded)
/// - `<hash>`: The actual password hash (base64-encoded)
///
/// **Why use salt?**
/// Salt is a random string appended to the password before hashing. This ensures:
/// 1. Same password → different hashes each time (prevents rainbow table attacks)
/// 2. Each user has a unique hash (can't identify users with same password)
/// 3. Pre-computed hash tables become useless
///
/// **Rainbow Table Attack Prevention:**
/// Without salt: `hash("password123")` always produces the same hash
/// Attacker can pre-compute millions of common passwords and look them up instantly.
///
/// With salt: `hash("password123" + random_salt)` produces different hash every time
/// Attacker must compute hash for each user individually - infeasible for large databases.
///
/// # Parameters
/// - `password`: The plain-text password to hash (String, &str, or anything Into<String>)
///
/// # Returns
/// - `Ok(String)`: The hashed password in PHC format (safe to store in database)
/// - `Err(ErrorMessage)`: If validation fails or hashing error occurs
///
/// # Security Notes
/// - NEVER store plain-text passwords in the database
/// - The hash includes the salt, so you only need to store the hash string
/// - Each call produces a different hash (even for the same password)
/// - This is expected behavior and doesn't affect verification
///
/// # Example
/// ```
/// let hashed = hash("my_secure_password")?;
/// // hashed = "$argon2id$v=19$m=19456,t=2,p=1$random_salt$hash_output"
/// // Store this entire string in the database
/// ```
pub fn hash(password: impl Into<String>) -> Result<String, ErrorMessage> {
    let password = password.into();

    // Validation: Reject empty passwords
    if password.is_empty() {
        return Err(ErrorMessage::EmptyPassword);
    }

    // Validation: Enforce maximum length to prevent DoS attacks
    // Argon2 is intentionally slow - very long passwords could cause timeouts
    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(ErrorMessage::ExceededMaxPasswordLength(MAX_PASSWORD_LENGTH));
    }

    // Generate a cryptographically secure random salt
    // OsRng uses the operating system's CSPRNG (Cryptographically Secure Pseudo-Random Number Generator)
    // - On Linux: /dev/urandom
    // - On Windows: BCryptGenRandom
    // - On macOS: SecRandomCopyBytes
    let salt = SaltString::generate(&mut OsRng);

    // Hash the password with Argon2id (default parameters)
    // Default parameters (as of argon2 crate v0.5+):
    // - Memory: 19 MB (m=19456 KiB)
    // - Iterations: 2 (t=2)
    // - Parallelism: 1 thread (p=1)
    // - Output length: 32 bytes
    //
    // Process:
    // 1. Combine password + salt
    // 2. Apply Argon2id algorithm (memory-hard function)
    // 3. Produce 32-byte hash
    // 4. Encode salt and hash as base64
    // 5. Format as PHC string: $argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>
    let hashed_password = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| ErrorMessage::HashingError)?
        .to_string();

    Ok(hashed_password)
}

/// Verify a password against a stored hash
///
/// This function performs constant-time comparison to prevent timing attacks.
///
/// **How Verification Works:**
/// 1. Parse the stored hash string to extract salt and parameters
/// 2. Hash the provided password with the SAME salt and parameters
/// 3. Compare the newly computed hash with the stored hash
/// 4. Return true only if they match exactly
///
/// **Why extract salt from hash?**
/// The PHC format stores everything needed to verify:
/// - Algorithm and version
/// - Cost parameters (memory, time, parallelism)
/// - Salt (in base64)
/// - Expected hash (in base64)
///
/// This means you only need to store one string per password!
///
/// **Timing Attack Prevention:**
/// The comparison is done in constant time - it takes the same amount of time
/// whether the password is correct or wrong. This prevents attackers from
/// learning information by measuring how long verification takes.
///
/// # Parameters
/// - `password`: The plain-text password to verify
/// - `hashed_password`: The stored hash (PHC format string from database)
///
/// # Returns
/// - `Ok(true)`: Password matches
/// - `Ok(false)`: Password doesn't match
/// - `Err(ErrorMessage)`: Validation error or invalid hash format
///
/// # Example
/// ```
/// // During registration:
/// let hashed = hash("user_password")?;
/// db.store_user(email, hashed)?;
///
/// // During login:
/// let stored_hash = db.get_password_hash(email)?;
/// if compare("user_password", &stored_hash)? {
///     // Login successful
/// } else {
///     // Wrong password
/// }
/// ```
pub fn compare(password: &str, hashed_password: &str) -> Result<bool, ErrorMessage> {
    // Validation: Reject empty passwords
    if password.is_empty() {
        return Err(ErrorMessage::EmptyPassword);
    }

    // Validation: Enforce maximum length (same as hashing)
    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(ErrorMessage::ExceededMaxPasswordLength(MAX_PASSWORD_LENGTH));
    }

    // Parse the PHC format hash string
    // This extracts:
    // - Algorithm identifier (argon2id)
    // - Version (v=19)
    // - Parameters (m, t, p)
    // - Salt (base64-decoded)
    // - Expected hash (base64-decoded)
    //
    // If the format is invalid (corrupted database, wrong algorithm, etc.),
    // this will fail and return InvalidHashFormat error
    let parsed_hash =
        PasswordHash::new(hashed_password).map_err(|_| ErrorMessage::InvalidHashFormat)?;

    // Verify the password against the parsed hash
    // Process:
    // 1. Extract salt and parameters from parsed_hash
    // 2. Hash the provided password with the same salt and parameters
    // 3. Compare hashes in constant time
    //
    // verify_password returns:
    // - Ok(()) if password matches
    // - Err(password_hash::Error) if password doesn't match or verification fails
    //
    // map_or transforms the Result:
    // - Ok(()) → true (password matches)
    // - Err(_) → false (password doesn't match)
    let password_matched = Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_or(false, |_| true);

    Ok(password_matched)
}
