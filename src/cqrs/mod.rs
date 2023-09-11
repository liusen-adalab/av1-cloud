use ::diesel::{deserialize::FromSqlRow, expression::AsExpression};
use actix_identity::Identity;
use actix_web::{web, HttpResponse};
use async_graphql::{
    http::GraphiQLSource, scalar, Context, EmptyMutation, EmptySubscription, InputObject, Object,
    Schema,
};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use serde::{Deserialize, Serialize};

pub mod file_system;
pub(crate) mod user;

pub fn actix_config(cfg: &mut web::ServiceConfig) {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();
    let schema_dev = Schema::build(AdminQueryRoot, EmptyMutation, EmptySubscription).finish();
    cfg.app_data(actix_web::web::Data::new(schema))
        .app_data(actix_web::web::Data::new(schema_dev))
        .service(
            web::resource("/api/query")
                .route(web::post().to(index))
                .route(web::get().to(playgroud)),
        )
        .service(
            web::resource("/admin/query")
                .route(web::post().to(index_dev))
                .route(web::get().to(playgroud_dev)),
        );
}

pub type Av1Schema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct QueryRoot;

#[Object]
/// 查询根节点
impl QueryRoot {
    async fn ping(&self) -> &'static str {
        "pong"
    }

    /// 获取用户
    async fn user(&self, ctx: &Context<'_>) -> async_graphql::Result<User> {
        let id = ctx.user_id_unchecked();
        let id = User::load(id).await?;
        Ok(id)
    }
}

pub trait UserIdCtxExt {
    fn user_id_unchecked(&self) -> UserId;
}

impl UserIdCtxExt for Context<'_> {
    fn user_id_unchecked(&self) -> UserId {
        *(self.data_unchecked::<UserId>())
    }
}

pub type AdminSchema = Schema<AdminQueryRoot, EmptyMutation, EmptySubscription>;
pub struct AdminQueryRoot;

#[Object]
/// 管理员查询根节点
impl AdminQueryRoot {
    async fn ping(&self) -> &'static str {
        "pong"
    }

    /// 获取用户
    async fn user(&self, id: String) -> async_graphql::Result<User> {
        let id = id.parse()?;
        let id = User::load(id).await?;
        Ok(id)
    }

    /// 获取用户列表
    async fn user_list(&self, params: UserSearchParams) -> async_graphql::Result<UserList> {
        Ok(User::list(params).await?)
    }
}

async fn index(
    schema: web::Data<Av1Schema>,
    req: GraphQLRequest,
    id: Option<Identity>,
) -> actix_web::Result<GraphQLResponse> {
    let mut req = req.into_inner();
    if id.is_none() {
        req = req.only_introspection();
        return Ok(schema.execute(req).await.into());
    }
    let id: i64 = id
        .unwrap()
        .id()
        .map_err(|err| -> Box<dyn std::error::Error> { format!("{}", err).into() })?
        .parse()
        .map_err(|err| -> Box<dyn std::error::Error> { format!("{}", err).into() })?;
    let req = req.data(UserId(id));
    Ok(schema.execute(req).await.into())
}

async fn playgroud() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/api/query").finish()))
}

async fn index_dev(
    schema: web::Data<AdminSchema>,
    req: GraphQLRequest,
) -> actix_web::Result<GraphQLResponse> {
    let req = req.into_inner();
    Ok(schema.execute(req).await.into())
}

async fn playgroud_dev() -> actix_web::Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/admin/query").finish()))
}

use derive_more::From;

use crate::domain::user::user::UserId;

use self::user::{User, UserList, UserSearchParams};

#[derive(Deserialize, From, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = ::diesel::sql_types::Timestamptz)]
pub struct MillionTimestamp(chrono::DateTime<chrono::Local>);
scalar!(MillionTimestamp);

impl Serialize for MillionTimestamp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.0.timestamp_millis())
    }
}

#[derive(Debug, InputObject)]
pub struct Paginate {
    /// 页码，从 1 开始
    pub page: u32,
    /// 每页大小
    pub page_size: u32,
}

impl Paginate {
    pub fn cursor(&self) -> Option<u32> {
        let page_idx = self.page.checked_sub(1)?;
        page_idx.checked_mul(self.page_size)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Address(Vec<String>);
scalar!(Address);

impl From<String> for Address {
    fn from(value: String) -> Self {
        Self(value.split(",").map(ToOwned::to_owned).collect())
    }
}

impl<'a> From<&'a Address> for String {
    fn from(value: &'a Address) -> Self {
        value.0.join(",")
    }
}

mod diesel_impl {
    #[macro_export]
    macro_rules! diesel_new_type {
        ($type:ty, $pg_type:ty) => {
            const _: () = {
                use diesel::{
                    backend::Backend,
                    deserialize::{self, FromSql},
                    pg::Pg,
                    serialize::{self, Output, ToSql},
                };
                impl ToSql<$pg_type, Pg> for $type {
                    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
                        ToSql::<$pg_type, Pg>::to_sql(&self.0, out)
                    }
                }

                impl FromSql<$pg_type, Pg> for $type {
                    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                        let res = FromSql::<$pg_type, Pg>::from_sql(bytes)?;
                        Ok(Self(res))
                    }
                }
            };
        };

        ($type:ty, $pg_type:ty, map_to: $map_ty:ty) => {
            const _: () = {
                use diesel::{
                    backend::Backend,
                    deserialize::{self, FromSql},
                    pg::Pg,
                    serialize::{self, Output, ToSql},
                };

                impl ToSql<$pg_type, Pg> for $type {
                    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
                        let map_to = <$map_ty>::from(self);
                        ToSql::<$pg_type, Pg>::to_sql(&map_to, &mut out.reborrow())
                    }
                }

                impl FromSql<$pg_type, Pg> for $type {
                    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                        let res: $map_ty = FromSql::<$pg_type, Pg>::from_sql(bytes)?;
                        Ok(Self::from(res))
                    }
                }
            };
        };
    }

    diesel_new_type!(super::MillionTimestamp, diesel::pg::sql_types::Timestamptz);
    diesel_new_type!(super::Address, diesel::sql_types::Text, map_to: String);
}
