use anyhow::{bail, ensure};
use async_graphql::{ComplexObject, Enum, InputObject, SimpleObject};
use chrono::NaiveDateTime;
use diesel::helper_types::IntoBoxed;
use diesel::TextExpressionMethods;
use diesel::{prelude::Queryable, QueryDsl, Selectable};
use diesel::{result::OptionalExtension, ExpressionMethods, SelectableHelper};
use diesel_async::RunQueryDsl;
use utils::db_pools::postgres::pg_conn;

use crate::schema::users;

use super::{FlakeId, MillionTimestamp, Paginate};

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
    /// 地址
    pub address: Option<super::Address>,
}

#[ComplexObject]
impl User {
    pub async fn level(&self) -> async_graphql::Result<UserLevel> {
        Ok(UserLevel::Normal)
    }

    pub async fn status(&self) -> async_graphql::Result<UserStatus> {
        Ok(UserStatus::Ok)
    }
}

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

    pub async fn list(params: UserSearchParams) -> anyhow::Result<UserList> {
        let Some(offset) = params.page.cursor() else {
            return Ok(Default::default());
        };

        let conn = &mut pg_conn().await?;

        let total: i64 = users::table.count().get_result(conn).await?;
        let mut sql = users::table.into_boxed();

        // where clause
        if let Some(search) = params.search_by {
            macro_rules! filter_if_not_empty {
                ($field:tt $(,)?) => {{
                    if let Some(field) = search.$field {
                        sql = sql.filter(users::$field.like(format!("%{}%", field)));
                    }
                }};
            }
            filter_if_not_empty!(name);
            filter_if_not_empty!(email);
            filter_if_not_empty!(mobile_number);

            macro_rules! interval_filter {
                ($search_field:tt, $sql_field:tt) => {
                    if let Some(interval) = search.$search_field {
                        let Some(start) = NaiveDateTime::from_timestamp_millis(interval.start_ms) else {
                            bail!("invalid timestamp: {}",interval.start_ms);
                        };
                        let Some(end) = NaiveDateTime::from_timestamp_millis(interval.end_ms) else {
                            bail!("invalid timestamp: {}",interval. end_ms);
                        };
                        sql = sql.filter(users::$sql_field.between(start, end));
                    }
                };
            }
            interval_filter!(latest_login, last_login);
            interval_filter!(register_at, create_at);
        }

        // order by
        let sql = params.sort.set_order_by(sql);

        let users: Vec<User> = sql
            .select(User::as_select())
            .offset(offset as i64)
            .limit(params.page.page_size as i64)
            .get_results(conn)
            .await?;
        Ok(UserList { total, users })
    }
}

#[derive(Default, SimpleObject)]
pub struct UserList {
    total: i64,
    users: Vec<User>,
}

#[derive(InputObject)]
pub struct UserSearchParams {
    /// 搜索条件，为空时不过滤
    search_by: Option<SearchBy>,
    /// 排序条件
    sort: Sort,
    /// 分页条件
    page: Paginate,
}

#[derive(InputObject)]
pub struct Sort {
    /// 排序字段
    by: SortBy,
    direction: Direction,
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// 按用户名排序
    Name,
    /// 按邮箱排序
    Email,
    /// 按手机号排序
    MobileNumber,
    /// 按最近登录时间排序
    LatestLogin,
    /// 按注册时间排序
    RegisterAt,
}

impl Sort {
    fn set_order_by<'a>(
        &self,
        sql: IntoBoxed<'a, users::table, diesel::pg::Pg>,
    ) -> IntoBoxed<'a, users::table, diesel::pg::Pg> {
        match self.direction {
            Direction::Up => match self.by {
                SortBy::Name => sql.order_by(users::name.asc()),
                SortBy::Email => sql.order_by(users::email.asc()),
                SortBy::MobileNumber => sql.order_by(users::mobile_number.asc()),
                SortBy::LatestLogin => sql.order_by(users::last_login.asc()),
                SortBy::RegisterAt => sql.order_by(users::create_at.asc()),
            },
            Direction::Down => match self.by {
                SortBy::Name => sql.order_by(users::name.desc()),
                SortBy::Email => sql.order_by(users::email.desc()),
                SortBy::MobileNumber => sql.order_by(users::mobile_number.desc()),
                SortBy::LatestLogin => sql.order_by(users::last_login.desc()),
                SortBy::RegisterAt => sql.order_by(users::create_at.desc()),
            },
        }
    }
}

#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// 升序
    Up,
    /// 降序
    Down,
}

#[derive(InputObject, Default)]
pub struct SearchBy {
    name: Option<String>,
    email: Option<String>,
    mobile_number: Option<String>,
    latest_login: Option<TimeInterval>,
    register_at: Option<TimeInterval>,
    level: Option<UserLevel>,
    status: Option<UserStatus>,
}

#[derive(InputObject)]
pub struct TimeInterval {
    start_ms: i64,
    end_ms: i64,
}

#[repr(i16)]
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum UserLevel {
    /// 普通用户
    Normal,
    Vip,
    Svip,
}

impl UserLevel {
    pub fn from_i16(value: i16) -> anyhow::Result<Self> {
        ensure!(value <= Self::Svip as i16, "invalid user level: {}", value);
        unsafe { Ok(std::mem::transmute(value)) }
    }
}

#[repr(i16)]
#[derive(Enum, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    /// 正常
    Ok,
    /// 封禁
    Baned,
}

impl UserStatus {
    pub fn from_i16(value: i16) -> anyhow::Result<Self> {
        ensure!(
            value <= UserStatus::Ok as i16,
            "invalid user status: {}",
            value
        );
        unsafe { Ok(std::mem::transmute(value)) }
    }
}
