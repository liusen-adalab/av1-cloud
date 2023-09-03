use std::borrow::Cow;

use crate::domain::user::user::UserId;
use crate::redis_conn_switch::redis_conn;
use crate::{
    domain::user::{
        employee::{Employee, EmployeeId, InviteCode},
        Email, Phone,
    },
    schema::employees,
};
use anyhow::Result;
use chrono::NaiveDateTime;
use diesel::{
    prelude::{Identifiable, Insertable, Queryable},
    query_builder::AsChangeset,
    result::OptionalExtension,
    Selectable,
};
use diesel::{ExpressionMethods, QueryDsl, SelectableHelper};
use diesel_async::RunQueryDsl;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use utils::db_pools::postgres::PgConn;

use super::EffectedRow;

#[derive(
    Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug, Serialize, Deserialize,
)]
#[diesel(table_name = employees)]
pub struct EmployeePo<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub mobile_number: Option<Cow<'a, str>>,
    pub email: Cow<'a, str>,
    pub password: Cow<'a, str>,
    pub last_login: NaiveDateTime,
    pub invited_by: i64,
    pub role: i16,
}

impl<'a> EmployeePo<'a> {
    fn from_do(user: &'a Employee) -> Self {
        Self {
            id: *user.id() as i64,
            name: Cow::Borrowed(&user.name()),
            mobile_number: user.mobile_number().as_ref().map(|p| Cow::Borrowed(&***p)),
            email: Cow::Borrowed(&user.email()),
            password: Cow::Borrowed(user.password().hashed_str()),
            last_login: *user.login_at(),
            invited_by: *user.invited_by(),
            role: *user.role() as i16,
        }
    }
}

#[derive(derive_more::From, Debug)]
pub(crate) enum EmployeeFindId<'a> {
    Email(&'a Email),
    Id(EmployeeId),
    Phone(&'a Phone),
}

pub(crate) async fn find<'a, T>(id: T, conn: &mut PgConn) -> Result<Option<Employee>>
where
    EmployeeFindId<'a>: From<T>,
{
    macro_rules! get_result {
        ($filter:expr) => {{
            let user: Option<EmployeePo> = employees::table
                .filter($filter)
                .select(EmployeePo::as_select())
                .for_update()
                .get_result(conn)
                .await
                .optional()?;
            user.map(|u| Employee::from_po(u)).transpose()
        }};
    }

    match EmployeeFindId::from(id) {
        EmployeeFindId::Email(email) => {
            get_result!(employees::email.eq(&**email))
        }
        EmployeeFindId::Id(id) => {
            get_result!(employees::id.eq(id))
        }
        EmployeeFindId::Phone(phone) => {
            get_result!(employees::mobile_number.eq(&**phone))
        }
    }
}

pub(crate) async fn save(user: &Employee, conn: &mut PgConn) -> Result<EffectedRow> {
    let user = EmployeePo::from_do(user);

    let effected = diesel::insert_into(employees::table)
        .values(&user)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;

    Ok(EffectedRow(effected))
}

pub(crate) async fn update(user: &Employee, conn: &mut PgConn) -> Result<()> {
    let user = EmployeePo::from_do(user);
    diesel::update(employees::table)
        .filter(employees::id.eq(user.id))
        .set(&user)
        .execute(conn)
        .await?;
    Ok(())
}

fn invite_code_key() -> String {
    format!("inter_user:invite_code")
}

pub(crate) async fn save_invite_code(invitor: UserId, invite_code: &InviteCode) -> Result<()> {
    let conn = &mut redis_conn().await?;
    conn.hset(invite_code_key(), invite_code.as_str(), invitor)
        .await?;
    Ok(())
}

pub(crate) async fn get_invitor_id(code: &InviteCode) -> Result<Option<EmployeeId>> {
    let conn = &mut redis_conn().await?;
    let invitor = conn.hget(invite_code_key(), code.as_str()).await?;
    Ok(invitor)
}
