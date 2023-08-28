use std::borrow::Cow;

use crate::{
    domain::{
        self,
        user::{Email, User, UserId},
    },
    schema::users,
};
use anyhow::Result;
use chrono::NaiveDateTime;
use diesel::{
    AsChangeset, ExpressionMethods, Identifiable, Insertable, QueryDsl, Queryable, Selectable,
    SelectableHelper,
};
use diesel_async::RunQueryDsl;
use utils::db_pools::postgres::PgConn;

#[derive(Queryable, Selectable, Insertable, AsChangeset, Identifiable)]
#[diesel(table_name = users)]
pub struct UserPo<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub mobile_number: Option<String>,
    pub email: Cow<'a, str>,
    pub password: Cow<'a, str>,
    pub address: Option<Cow<'a, str>>,
    pub last_login: NaiveDateTime,
    pub online: bool,
}

#[must_use]
pub struct EffectedRow(usize);

impl EffectedRow {
    pub fn actually_effected(&self) -> bool {
        self.0 > 0
    }
}

pub async fn save(user: &User, conn: &mut PgConn) -> Result<EffectedRow> {
    let user = UserPo::from_do(user);

    let effected = diesel::insert_into(users::table)
        .values(user)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;

    Ok(EffectedRow(effected))
}

pub(crate) async fn save_changed(user: &User, conn: &mut PgConn) -> Result<()> {
    let user = UserPo::from_do(user);
    diesel::update(users::table).set(user).execute(conn).await?;
    Ok(())
}

impl<'a> UserPo<'a> {
    fn from_do(user: &'a User) -> Self {
        Self {
            id: *user.id() as i64,
            name: Cow::Borrowed(&user.name()),
            mobile_number: None,
            email: Cow::Borrowed(&user.email()),
            password: Cow::Borrowed(user.password().hashed_str()),
            address: None,
            last_login: *user.login_at(),
            online: *user.online(),
        }
    }
}

#[derive(derive_more::From, Debug)]
pub enum UserFindId<'a> {
    Email(&'a Email),
    Id(UserId),
}

use diesel::result::OptionalExtension;

pub(crate) async fn find<'a, T>(id: T, conn: &mut PgConn) -> Result<Option<User>>
where
    UserFindId<'a>: From<T>,
{
    macro_rules! get_result {
        ($filter:expr) => {{
            let user: Option<UserPo> = users::table
                .filter($filter)
                .select(UserPo::as_select())
                .for_update()
                .get_result(conn)
                .await
                .optional()?;
            user.map(|u| domain::user::po_to_do(u)).transpose()
        }};
    }

    match UserFindId::from(id) {
        UserFindId::Email(email) => {
            get_result!(users::email.eq(&**email))
        }
        UserFindId::Id(id) => {
            get_result!(users::id.eq(id))
        }
    }
}