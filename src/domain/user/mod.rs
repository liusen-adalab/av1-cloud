use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use tracing::warn;

use crate::ensure_ok;

pub mod employee;
pub mod service;
pub mod user;

pub use common_err::*;

mod common_err {
    pub enum SanityCheck {
        EmailCodeNotMatch,
        SmsCodeNotMatch,
        PasswordNotMatch,
    }
}

#[derive(derive_more::Deref, Debug)]
pub struct UserName(String);

#[derive(derive_more::Deref, Debug, Clone)]
pub struct Email(String);

#[derive(Debug)]
pub struct Password(String);

#[derive(derive_more::Deref, Debug)]
pub struct Phone(String);

#[derive(Display, Debug)]
pub struct PhoneFormatErr;

impl std::error::Error for PhoneFormatErr {}

impl Phone {
    pub fn try_from(phone: String) -> Result<Self, PhoneFormatErr> {
        // TODO: check phone format
        Ok(Self(phone))
    }
}

use derive_more::Display;

#[derive(Debug, Display)]
pub enum UserNameFormatErr {
    TooLong,
    TooShort,
    NotAllowedChar,
}

impl std::error::Error for UserNameFormatErr {}

impl UserName {
    pub fn try_from(value: String) -> Result<Self, UserNameFormatErr> {
        ensure_ok!(value.len() >= 2, UserNameFormatErr::TooShort);
        ensure_ok!(value.len() < 16, UserNameFormatErr::TooLong);

        // ensure_ok!(
        //     value.chars().all(|c| c.is_alphanumeric()),
        //     UserNameFormatErr::NotAllowedChar
        // );

        Ok(Self(value))
    }
}
#[derive(Debug, derive_more::Display)]
pub enum EmailFormatErr {
    Invalid,
}

impl std::error::Error for EmailFormatErr {}

impl Email {
    pub fn try_from(value: String) -> Result<Self, EmailFormatErr> {
        ensure_ok!(
            email_address::EmailAddress::is_valid(&value),
            EmailFormatErr::Invalid
        );
        Ok(Self(value))
    }
}

#[derive(Debug, derive_more::Display)]
pub enum PasswordFormatErr {
    TooLong,
    TooShort,
    NotAllowedChar,
    TooSimple,
}

impl Password {
    pub async fn try_from_async(value: String) -> Result<Self, PasswordFormatErr> {
        ensure_ok!(value.len() >= 8, PasswordFormatErr::TooShort);
        ensure_ok!(value.len() <= 20, PasswordFormatErr::TooLong);
        ensure_ok!(value.is_ascii(), PasswordFormatErr::NotAllowedChar);
        ensure_ok!(!Self::is_monotonic(&value, 5), PasswordFormatErr::TooSimple);

        let value = tokio::task::spawn_blocking(|| Self::encrypt_password(value))
            .await
            .unwrap()
            .map_err(|err| {
                warn!(?err, "failed to hash password");
                PasswordFormatErr::NotAllowedChar
            })?;
        Ok(Self(value))
    }

    pub fn hashed_str(&self) -> &str {
        &self.0
    }

    pub async fn verify(&self, password: &str) -> bool {
        let hashsed = &PasswordHash::new(&self.0).unwrap();
        Argon2::default()
            .verify_password(password.as_bytes(), hashsed)
            .is_ok()
    }

    /// 检查字符串中字符的单调性，判断 `str` 中最长的连续单调子字串长度是否大于 `limit`
    ///
    /// # Panics
    /// `limit == 0` 时 panic
    fn is_monotonic(str: &str, limit: usize) -> bool {
        #[derive(PartialEq, Eq, Clone, Copy)]
        enum Monotonic {
            Rise,
            Decline,
            Equal,
            None,
        }
        assert_ne!(limit, 0);
        if str.len() <= limit {
            return false;
        }

        let get_mono = |a: u8, b: u8| match a as i16 - b as i16 {
            0 => Monotonic::Equal,
            1 => Monotonic::Rise,
            -1 => Monotonic::Decline,
            _ => Monotonic::None,
        };

        let bytes = str.as_bytes();

        // SAFETY: 经过前面的检查，长度至少为 2
        // let mut last_derection = get_mono(bytes[1], bytes[0]);
        let mut last_derection = Monotonic::None;
        let mut monotonic_len = 1;

        for (pre, aft) in bytes.iter().zip(bytes.iter().skip(1)) {
            let mono = get_mono(*aft, *pre);
            match (last_derection, mono) {
                (_, Monotonic::None) => {
                    monotonic_len = 1;
                }
                (Monotonic::None, _) => {
                    monotonic_len = 2;
                }
                (_, _) => {
                    if last_derection == mono {
                        monotonic_len += 1;
                    } else {
                        monotonic_len = 2;
                    }
                }
            }
            last_derection = mono;

            if monotonic_len > limit {
                return true;
            }
        }

        false
    }

    pub async fn encrypt_password_async(password: String) -> Result<String, anyhow::Error> {
        // 由于加密特性，这个函数的调用时间大约为 340 ms
        // 将这个函数放入专用的线程池中，防止阻塞其他异步任务
        let value = tokio::task::spawn_blocking(|| Self::encrypt_password(password))
            .await
            .unwrap()
            .map_err(|err| {
                warn!(?err, "failed to hash password");
                err
            })?;
        Ok(value)
    }

    pub fn encrypt_password(password: String) -> Result<String, anyhow::Error> {
        // hash password
        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        // Argon2 with default params (Argon2id v19)
        let argon2 = Argon2::default();
        // Hash password to PHC string ($argon2id$v=19$...)
        Ok(argon2.hash_password(password.as_ref(), &salt)?.to_string())
    }
}
