use anyhow::bail;
use async_graphql::{ComplexObject, SimpleObject};
use diesel::{prelude::Queryable, QueryDsl, Selectable};
use diesel::{result::OptionalExtension, ExpressionMethods, SelectableHelper};
use diesel_async::RunQueryDsl;
use utils::db_pools::postgres::pg_conn;

use crate::schema::users;

use super::{FlakeId, MillionTimestamp};

pub type UserId = i64;

#[derive(Queryable, Selectable, SimpleObject)]
#[graphql(complex)]
/// 用户节点
pub struct User {
    pub id: FlakeId,
    /// 用户名
    pub name: String,
    /// 手机号
    pub mobile_number: Option<String>,
    /// 邮箱
    pub email: String,
    /// 最近登录时间
    pub last_login: MillionTimestamp,
    /// 注册时间
    pub create_at: MillionTimestamp,
    /// 是否在线
    pub online: bool,
}

#[ComplexObject]
impl User {}

impl User {
    pub async fn load(id: UserId) -> anyhow::Result<User> {
        let user = Self::load_may_none(id).await?;

        if let Some(user) = user {
            Ok(user)
        } else {
            bail!("user not found, id = {}", id)
        }
    }

    pub async fn load_may_none(id: UserId) -> anyhow::Result<Option<User>> {
        let conn = &mut pg_conn().await?;
        let user: Option<User> = users::table
            .filter(users::id.eq(id))
            .select(User::as_select())
            .for_update()
            .get_result(conn)
            .await
            .optional()?;
        Ok(user)
    }
}
