use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash,
        PasswordHasher,
        PasswordVerifier,
        SaltString
    },
    Argon2,
};

use crate::error::ErrorMessage;

const MAX_PASSWORD_LENGTH: usize = 64;

pub fn hash(password: impl Into<String>) -> Result<String, ErrorMessage> {
    let password = password.into();

    if password.is_empty() {
        return Err(ErrorMessage::EmptyPassword);
    }

    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(ErrorMessage::ExceededMaxPasswordLength(MAX_PASSWORD_LENGTH));
    }

    let salt = SaltString::generate(&mut OsRng);
    //salt는 password뒤에 추가하는 임의의 문자열(랜덤), 이를 통해 보안 성능을 높힘.
    let hashed_password = Argon2::default()
            .hash_password(password.as_bytes(), &salt)//hash할 때 $argon2id$v=19$m=4096,t=3,p=1$<salt>$<hash> 뭐 이런식으로 바꾸는 듯 그런데 <salt>, <hash>부분이 이제 
            //<hash>부분이 password+salt를 hash하고 base64로 인코딩한 부분. <salt>부분은 그냥 salt를 base64로 인코딩한 부분. 이를 통해 같은 비밀번호라도 hash값이 매번 다름. (rainbox table 공격 방지)
            .map_err(|_| ErrorMessage::HashingError)?
            .to_string();

    Ok(hashed_password)
}

pub fn compare(password: &str, hashed_password: &str) -> Result<bool, ErrorMessage> {
    if password.is_empty() {
        return Err(ErrorMessage::EmptyPassword);
    }

    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(ErrorMessage::ExceededMaxPasswordLength(MAX_PASSWORD_LENGTH));
    }

    let parsed_hash = PasswordHash::new(hashed_password)
            .map_err(|_| ErrorMessage::InvalidHashFormat)?;

    let password_matched = Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_or(false, |_| true);

    Ok(password_matched)
}