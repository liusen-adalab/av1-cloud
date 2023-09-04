use chrono::NaiveDateTime;
use getset::Getters;
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use flaken::Flaken;

use crate::{
    biz_ok, domain::user::common_err::SanityCheck, ensure_biz, ensure_ok, http::BizResult,
    infrastructure::repo_user::UserPo,
};

use super::{
    service::{UpdateProfileErr, UserUpdate},
    Email, Password, Phone, UserName,
};

pub type UserId = i64;

#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct User {
    #[getset(get_copy)]
    id: UserId,
    name: UserName,
    email: Email,
    password: Password,
    mobile_number: Option<Phone>,
    address: Option<String>,
    online: bool,

    login_at: NaiveDateTime,
}

impl User {
    pub fn create(email: Email, password: Password) -> Self {
        Self {
            id: Self::next_id() as i64,
            name: UserName::try_from("user".to_string()).unwrap(),
            email,
            password,
            login_at: Local::now().naive_local(),
            mobile_number: None,
            address: None,
            online: true,
        }
    }

    pub async fn login(&mut self, password: &str) -> BizResult<(), SanityCheck> {
        ensure_biz!(
            self.password.verify(password).await,
            SanityCheck::PasswordNotMatch
        );
        self.login_at = Local::now().naive_local();
        self.online = true;
        biz_ok!(())
    }

    pub fn logout(&mut self) {
        self.online = false
    }

    pub fn reset_password(&mut self, new: Password) {
        self.password = new
    }

    pub async fn set_password(&mut self, old: String, new: Password) -> Result<(), SanityCheck> {
        ensure_ok!(
            self.password.verify(&old).await,
            SanityCheck::PasswordNotMatch
        );
        self.reset_password(new);
        Ok(())
    }

    pub async fn update_profile(&mut self, update: UserUpdate) -> BizResult<(), UpdateProfileErr> {
        if let Some(password) = update.password {
            ensure_biz!(
                self.set_password(password.old_password, password.new_password)
                    .await
            )
        }

        if let Some(name) = update.user_name {
            self.name = name
        }

        self.address = update.address;

        if let Some(mobile_number) = update.mobile_number {
            self.mobile_number = Some(mobile_number)
        }

        biz_ok!(())
    }

    fn next_id() -> u64 {
        static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
        let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
        let mut lock = f.lock().unwrap();
        lock.next()
    }

    pub fn from_po(user: UserPo) -> anyhow::Result<User> {
        Ok(User {
            id: user.id,
            name: UserName::try_from(user.name.into_owned())?,
            email: Email::try_from(user.email.into_owned())?,
            password: Password(user.password.into_owned()),
            login_at: user.last_login,
            mobile_number: user
                .mobile_number
                .map(|n| Phone::try_from(n.into_owned()))
                .transpose()?,
            address: user.address.map(|a| a.into_owned()),
            online: user.online,
        })
    }
}
