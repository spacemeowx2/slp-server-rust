use async_trait::async_trait;
use sha1::{Sha1, Digest};

#[derive(Debug, Clone, PartialEq)]
pub enum AuthError {
    NoSuchUser(String),
}

pub type AuthResult<T> = Result<T, AuthError>;

#[async_trait]
pub trait AuthProvider {
    /// Returns true if the user existed and password was correct
    /// Returns false if the password was incorrect
    /// Returns error if the user was not found
    async fn verify(&self, username: &str, challenge: &[u8], response: &[u8]) -> AuthResult<bool>;
}

#[async_trait]
pub trait PasswordAuthProvider {
    async fn get_user_password_sha1(&self, username: &str) -> AuthResult<Vec<u8>>;
}

pub fn sha1(message: &[u8]) -> Vec<u8> {
    Vec::from(&Sha1::digest(message)[..])
}

#[async_trait]
impl<T: PasswordAuthProvider + Sync> AuthProvider for T {
    async fn verify(&self, username: &str, challenge: &[u8], response: &[u8]) -> AuthResult<bool> {
        let mut password_sha1 = self.get_user_password_sha1(username).await?;
        password_sha1.extend_from_slice(challenge);
        Ok(sha1(&password_sha1) == response)
    }
}
