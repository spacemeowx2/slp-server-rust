use super::{AuthResult, AuthError, PasswordAuthProvider, sha1};

pub struct SimpleAuthProvider {
    username: String,
    password_sha1: Vec<u8>,
}

impl SimpleAuthProvider {
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            username: username.to_owned(),
            password_sha1: sha1(password.as_bytes()),
        }
    }
    pub fn new_unpw(unpw: &str) -> Self {
        let unpw: Vec<_> = unpw.splitn(2, ":").collect();
        Self::new(
            unpw[0],
            unpw.get(1).unwrap_or(&""),
        )
    }
}

#[async_trait::async_trait]
impl PasswordAuthProvider for SimpleAuthProvider {
    async fn get_user_password_sha1(&self, username: &str) -> AuthResult<Vec<u8>> {
        if username == self.username {
            Ok(self.password_sha1.clone())
        } else {
            Err(AuthError::NoSuchUser(username.to_string()))
        }
    }
}
