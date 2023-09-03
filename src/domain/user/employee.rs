use std::sync::{Mutex, OnceLock};

use anyhow::bail;
use chrono::{Local, NaiveDateTime};
use derive_more::*;
use flaken::Flaken;
use getset::Getters;

use crate::{
    domain::user::common_err::SanityCheck, ensure_ok, infrastructure::repo_employee::EmployeePo,
};

use super::{Email, Password, Phone, UserName};

pub type EmployeeId = i64;

#[derive(Getters, Debug)]
#[getset(get = "pub(crate)")]
pub struct Employee {
    id: EmployeeId,
    name: UserName,
    role: Role,
    invited_by: EmployeeId,
    email: Email,
    password: Password,
    mobile_number: Option<Phone>,

    login_at: NaiveDateTime,
}

#[repr(i16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Role {
    Employee,
    Manager,
    Admin,
}

impl TryFrom<i16> for Role {
    type Error = anyhow::Error;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Employee),
            1 => Ok(Self::Manager),
            2 => Ok(Self::Admin),
            _ => bail!("invalid role value: {}", value),
        }
    }
}

#[derive(From, AsRef, Deref, Debug)]
pub struct InviteCode(String);

impl InviteCode {
    pub(crate) fn generate() -> Self {
        todo!()
    }
}

impl Employee {
    pub fn create(email: Email, password: Password, invitor: EmployeeId) -> Self {
        Self {
            id: Self::next_id(),
            name: UserName::try_from("default-employee".to_string()).unwrap(),
            password,
            login_at: Local::now().naive_local(),
            mobile_number: None,
            email,
            role: Role::Employee,
            invited_by: invitor,
        }
    }

    pub async fn login(&mut self, password: &str) -> Result<(), SanityCheck> {
        ensure_ok!(
            self.password.verify(password).await,
            SanityCheck::PasswordNotMatch
        );
        self.login_at = Local::now().naive_local();
        Ok(())
    }

    pub fn logout(&mut self) {}

    fn next_id() -> i64 {
        static USER_ID_GENERATOR: OnceLock<Mutex<Flaken>> = OnceLock::new();
        let f = USER_ID_GENERATOR.get_or_init(|| Mutex::new(Flaken::default()));
        let mut lock = f.lock().unwrap();
        lock.next() as i64
    }

    pub fn from_po(user: EmployeePo) -> anyhow::Result<Employee> {
        Ok(Employee {
            id: user.id,
            name: UserName::try_from(user.name.into_owned())?,
            email: Email::try_from(user.email.into_owned())?,
            password: Password(user.password.into_owned()),
            login_at: user.last_login,
            mobile_number: user
                .mobile_number
                .map(|n| Phone::try_from(n.into_owned()))
                .transpose()?,
            role: Role::try_from(user.role)?,
            invited_by: user.invited_by,
        })
    }
}
