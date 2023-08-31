use std::borrow::Cow;

use crate::{
    domain::{
        self,
        user::{Email, Phone, User, UserId},
    },
    redis_conn_switch::redis_conn,
    schema::users,
};
use anyhow::Result;
use chrono::NaiveDateTime;
use diesel::{
    AsChangeset, ExpressionMethods, Identifiable, Insertable, QueryDsl, Queryable, Selectable,
    SelectableHelper,
};
use diesel_async::RunQueryDsl;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use utils::db_pools::postgres::PgConn;

#[derive(
    Queryable, Selectable, Insertable, AsChangeset, Identifiable, Debug, Serialize, Deserialize,
)]
#[diesel(table_name = users)]
pub struct UserPo<'a> {
    pub id: i64,
    pub name: Cow<'a, str>,
    pub mobile_number: Option<Cow<'a, str>>,
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

pub(crate) async fn save(user: &User, conn: &mut PgConn) -> Result<EffectedRow> {
    let user = UserPo::from_do(user);

    let effected = diesel::insert_into(users::table)
        .values(&user)
        .on_conflict_do_nothing()
        .execute(conn)
        .await?;

    // 记录已注册邮箱
    let _: () = redis_conn()
        .await?
        .sadd(registered_email_record_key(), user.email.as_ref())
        .await?;

    Ok(EffectedRow(effected))
}

pub(crate) async fn update(user: &User, conn: &mut PgConn) -> Result<()> {
    let user = UserPo::from_do(user);
    diesel::update(users::table)
        .filter(users::id.eq(user.id))
        .set(&user)
        .execute(conn)
        .await?;
    // 更新后删除缓存
    redis_conn().await?.del(user_key(&user.email)).await?;
    Ok(())
}

impl<'a> UserPo<'a> {
    fn from_do(user: &'a User) -> Self {
        Self {
            id: *user.id() as i64,
            name: Cow::Borrowed(&user.name()),
            mobile_number: user.mobile_number().as_ref().map(|p| Cow::Borrowed(&***p)),
            email: Cow::Borrowed(&user.email()),
            password: Cow::Borrowed(user.password().hashed_str()),
            address: user.address().as_ref().map(|a| Cow::Borrowed(&**a)),
            last_login: *user.login_at(),
            online: *user.online(),
        }
    }
}

#[derive(derive_more::From, Debug)]
pub enum UserFindId<'a> {
    Email(&'a Email),
    Id(UserId),
    Phone(&'a Phone),
}

use diesel::result::OptionalExtension;

fn registered_email_record_key() -> &'static str {
    "user:registerd_emails"
}

fn user_key(email: &str) -> String {
    format!("user:obj:{}", email)
}

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
            {
                // 先查缓存
                let mut r_conn = redis_conn().await?;
                if let Some(user) = r_conn.get::<_, Option<String>>(user_key(email)).await? {
                    let user: UserPo = serde_json::from_str(&user)?;
                    return Ok(Some(domain::user::po_to_do(user)?));
                }
            }

            let user = get_result!(users::email.eq(&**email));

            {
                // 如果找到，写入缓存，有效期 5 分钟
                if let Ok(Some(user)) = &user {
                    let user_po = UserPo::from_do(user);
                    let _: () = redis_conn()
                        .await?
                        .set_ex(
                            user_key(email),
                            serde_json::to_string(&user_po).unwrap(),
                            60 * 5,
                        )
                        .await?;
                }
            }

            user
        }
        UserFindId::Id(id) => {
            get_result!(users::id.eq(id))
        }
        UserFindId::Phone(phone) => {
            get_result!(users::mobile_number.eq(&**phone))
        }
    }
}

pub(crate) async fn exist<'a, T>(id: T, conn: &mut PgConn) -> Result<bool>
where
    UserFindId<'a>: From<T>,
{
    macro_rules! is_exist {
        ($filter:expr, $conn:expr) => {{
            let exist = diesel::select(diesel::dsl::exists(users::table.filter($filter)))
                .get_result($conn)
                .await?;
            Ok(exist)
        }};
    }

    match UserFindId::from(id) {
        UserFindId::Email(email) => {
            let k_conn = &mut redis_conn().await?;
            let exist = k_conn
                .sismember(registered_email_record_key(), &**email)
                .await?;
            Ok(exist)
        }
        UserFindId::Id(id) => {
            is_exist!(users::id.eq(id), conn)
        }
        UserFindId::Phone(phone) => {
            is_exist!(users::mobile_number.eq(&**phone), conn)
        }
    }
}
