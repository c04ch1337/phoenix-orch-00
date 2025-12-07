use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use actix_web::{error::ErrorUnauthorized, Error, HttpRequest};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,  // Subject (user id)
    pub exp: u64,     // Expiration time
    pub iat: u64,     // Issued at
}

#[derive(Clone)]
pub struct JwtAuth {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl JwtAuth {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
        }
    }

    pub fn create_token(&self, user_id: &str, duration: Duration) -> Result<String, jsonwebtoken::errors::Error> {
        let now = OffsetDateTime::now_utc();
        let exp = now.checked_add(duration).unwrap();

        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.unix_timestamp() as u64,
            iat: now.unix_timestamp() as u64,
        };

        encode(&Header::default(), &claims, &self.encoding_key)
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(token, &self.decoding_key, &validation)?;
        Ok(token_data.claims)
    }
}

pub async fn verify_auth(req: &HttpRequest, jwt_auth: &JwtAuth) -> Result<Claims, Error> {
    let auth_header = req.headers()
        .get("Authorization")
        .ok_or_else(|| ErrorUnauthorized("Missing authorization header"))?;

    let auth_str = auth_header.to_str()
        .map_err(|_| ErrorUnauthorized("Invalid authorization header"))?;

    if !auth_str.starts_with("Bearer ") {
        return Err(ErrorUnauthorized("Invalid authorization scheme"));
    }

    let token = &auth_str[7..];
    jwt_auth.validate_token(token)
        .map_err(|_| ErrorUnauthorized("Invalid token"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

    #[test]
    fn test_jwt_token_creation_and_validation() {
        let secret = b"test_secret";
        let jwt_auth = JwtAuth::new(secret);
        let user_id = "test_user";
        let duration = Duration::hours(1);

        // Create token
        let token = jwt_auth.create_token(user_id, duration).unwrap();
        
        // Validate token
        let claims = jwt_auth.validate_token(&token).unwrap();
        assert_eq!(claims.sub, user_id);
    }
}