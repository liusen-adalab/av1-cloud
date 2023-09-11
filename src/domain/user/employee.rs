use anyhow::bail;
use chrono::{Local, NaiveDateTime};
use derive_more::*;
use getset::Getters;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::{
    domain::user::common_err::SanityCheck, ensure_ok, id_wraper,
    infrastructure::repo_employee::EmployeePo,
};

use super::{Email, Password, Phone, UserName};

id_wraper!(EmployeeId);

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
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Employee,
    Manager,
    Root,
}

impl TryFrom<i16> for Role {
    type Error = anyhow::Error;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Employee),
            1 => Ok(Self::Manager),
            2 => Ok(Self::Root),
            _ => bail!("invalid role value: {}", value),
        }
    }
}

#[derive(From, AsRef, Deref, Debug)]
pub struct InviteCode(String);

impl InviteCode {
    pub(crate) fn generate() -> Self {
        let code: i64 = thread_rng().gen_range(100000..999999);
        Self(code.to_string())
    }
}

impl Employee {
    pub fn create(email: Email, password: Password, invitor: EmployeeId) -> Self {
        Self {
            id: EmployeeId::next_id(),
            name: UserName::try_from("employee".to_string()).unwrap(),
            password,
            login_at: Local::now().naive_local(),
            mobile_number: None,
            email,
            role: Role::Employee,
            invited_by: invitor,
        }
    }

    pub fn set_role(&mut self, role: Role) {
        self.role = role;
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
