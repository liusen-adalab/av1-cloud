pub(crate) mod user;

use ::diesel::{deserialize::FromSqlRow, expression::AsExpression};
use actix_identity::Identity;
use actix_web::{web, HttpResponse};
use async_graphql::{
    http::GraphiQLSource, scalar, Context, EmptyMutation, EmptySubscription, Object, Schema,
};
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub fn actix_config(cfg: &mut web::ServiceConfig) {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription).finish();
    let schema_dev = Schema::build(AdminQueryRoot, EmptyMutation, EmptySubscription).finish();
    cfg.app_data(actix_web::web::Data::new(schema))
        .app_data(actix_web::web::Data::new(schema_dev))
        .service(
            web::resource("/api/query")
                .route(web::get().to(playgroud))
                .route(web::post().to(index)),
        )
        .service(
            web::resource("/api/query/dev")
                .route(web::post().to(index_dev))
                .route(web::get().to(playgroud_dev)),
        );
}

pub type Av1Schema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn ping(&self) -> &'static str {
        "pong"
    }

    async fn user(&self, ctx: &Context<'_>) -> async_graphql::Result<User> {
        let id = userid_unchecked(ctx);
        let id = User::load(id).await?;
        Ok(id)
    }
}

fn userid_unchecked(ctx: &Context<'_>) -> UserId {
    *(ctx.data_unchecked::<UserId>())
}

pub type AdminSchema = Schema<AdminQueryRoot, EmptyMutation, EmptySubscription>;
pub struct AdminQueryRoot;

#[Object]
impl AdminQueryRoot {
    async fn ping(&self) -> &'static str {
        "pong"
    }

    async fn user(&self, id: String) -> async_graphql::Result<User> {
        let id = id.parse()?;
        let id = User::load(id).await?;
        Ok(id)
    }
}

async fn index(
    schema: web::Data<Av1Schema>,
    req: GraphQLRequest,
    id: Identity,
) -> actix_web::Result<GraphQLResponse> {
    let req = req.into_inner();
    let id: i64 = id
        .id()
        .map_err(|err| -> Box<dyn std::error::Error> { format!("{}", err).into() })?
        .parse()
        .map_err(|err| -> Box<dyn std::error::Error> { format!("{}", err).into() })?;
    let req = req.data(id);
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
        .body(GraphiQLSource::build().endpoint("/api/query/dev").finish()))
}

use derive_more::From;

use self::user::{User, UserId};

#[derive(Deserialize, From, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = ::diesel::sql_types::BigInt)]
pub struct FlakeId(i64);
scalar!(FlakeId);

impl Serialize for FlakeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[derive(Deserialize, From, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = ::diesel::sql_types::Timestamp)]
pub struct MillionTimestamp(NaiveDateTime);
scalar!(MillionTimestamp);

impl Serialize for MillionTimestamp {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.0.timestamp_millis())
    }
}

mod diesel_impl {
    use diesel::{
        backend::Backend,
        deserialize::{self, FromSql},
        pg::Pg,
        serialize::{self, Output, ToSql},
    };

    use super::MillionTimestamp;

    macro_rules! diesel_new_type {
        ($type:ty, $pg_type:ty) => {
            impl ToSql<$pg_type, Pg> for $type {
                fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
                    ToSql::<$pg_type, Pg>::to_sql(&self.0, out)
                }
            }

            impl FromSql<$pg_type, Pg> for $type {
                fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> deserialize::Result<Self> {
                    let time = FromSql::<$pg_type, Pg>::from_sql(bytes)?;
                    Ok(Self(time))
                }
            }
        };
    }

    diesel_new_type!(MillionTimestamp, diesel::sql_types::Timestamp);
    diesel_new_type!(super::FlakeId, diesel::sql_types::BigInt);
}