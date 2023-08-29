use serde::Deserialize;
use utils::db_pools::postgres::pg_conn;

use crate::{
    biz_ok,
    domain::user::{
        service::{self, login_tx, LoginErr, RegisterErr},
        Email, EmailFormatErr, Password, User, UserId,
    },
    ensure_biz,
    http::BizResult,
    infrastructure::{self, repo_user},
    pg_tx, tx_func,
};
use anyhow::Result;

pub async fn is_email_registerd(email: String) -> Result<bool> {
    let Ok(email) = Email::try_from(email) else {
        return Ok(false);
    };
    let conn = &mut pg_conn().await?;
    let exist = repo_user::exist(&email, conn).await?;
    Ok(exist)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    email: String,
    email_code: String,
    password: String,
}

pub async fn register(user_dto: UserDto) -> BizResult<UserId, RegisterErr> {
    let email = ensure_biz!(Email::try_from(user_dto.email));
    let password = ensure_biz!(Password::try_from_async(user_dto.password).await);
    let user = User::create(email, password);
    service::register(user, user_dto.email_code).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginDto {
    email: String,
    password: String,
}

pub async fn login(login: LoginDto) -> BizResult<UserId, LoginErr> {
    let email = ensure_biz!(Email::try_from(login.email));
    pg_tx!(login_tx, email, login.password)
}

pub async fn logout(id: UserId) -> anyhow::Result<()> {
    tx_func!(service::logout_tx, id)
}

#[derive(derive_more::From)]
pub enum SendEmailCodeErr {
    Email(EmailFormatErr),
}

pub async fn send_email_code(email: String, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    infrastructure::email::send_code(&email, fake).await?;
    biz_ok!(())
}
