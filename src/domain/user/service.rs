use anyhow::bail;
use utils::db_pools::postgres::PgConn;

use crate::{
    biz_ok,
    domain::user::Email,
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{
        email::{self, EmailCodeSender},
        repo_user,
    },
    pg_tx,
};
use derive_more::From;

use super::{EmailFormatErr, Password, PasswordFormatErr, User, UserId, UserNameFormatErr};

#[derive(derive_more::From)]
pub enum RegisterErr {
    Name(UserNameFormatErr),
    Password(PasswordFormatErr),
    Email(EmailFormatErr),
    AlreadyRegister,
    EmailCodeMisMatch,
}

pub async fn register(user: User, email_code: String) -> BizResult<UserId, RegisterErr> {
    ensure_biz!(
        email::EmailCodeSender::verify_email_code(&user.email, &email_code).await?,
        RegisterErr::EmailCodeMisMatch
    );

    pg_tx!(register_tx, user)
}

pub async fn register_tx(user: User, conn: &mut PgConn) -> BizResult<UserId, RegisterErr> {
    ensure_biz!(
        repo_user::save(&user, conn).await?.actually_effected(),
        RegisterErr::AlreadyRegister
    );
    biz_ok!(user.id)
}

#[derive(derive_more::From)]
pub enum LoginErr {
    Password(PasswordFormatErr),
    Email(EmailFormatErr),
    EmailOrPasswordWrong,
}

pub async fn login_tx(
    email: Email,
    password: String,
    conn: &mut PgConn,
) -> BizResult<UserId, LoginErr> {
    let user = repo_user::find(&email, conn).await?;
    let mut user = ensure_exist!(user, LoginErr::EmailOrPasswordWrong);
    ensure_biz!(user.login(&password).await?);
    repo_user::update(&user, conn).await?;

    biz_ok!(user.id)
}

pub async fn logout_tx(user_id: UserId, conn: &mut PgConn) -> anyhow::Result<()> {
    let Some(mut user) = repo_user::find(user_id, conn).await? else {
        bail!("user not found. id = {}", user_id);
    };

    user.logout();
    repo_user::update(&user, conn).await?;

    Ok(())
}

#[derive(From)]
pub enum ResetPasswordErr {
    Password(PasswordFormatErr),
    Email(EmailFormatErr),
    CodeNotMatch,
    NotFound,
}

pub async fn reset_password(
    email: Email,
    new_password: Password,
    email_code: String,
) -> BizResult<(), ResetPasswordErr> {
    ensure_biz!(
        EmailCodeSender::verify_email_code(&email, &email_code).await?,
        ResetPasswordErr::CodeNotMatch
    );
    pg_tx!(reset_password_tx, email, new_password)
}

pub async fn reset_password_tx(
    email: Email,
    new_password: Password,
    conn: &mut PgConn,
) -> BizResult<(), ResetPasswordErr> {
    let mut user = ensure_exist!(
        repo_user::find(&email, conn).await?,
        ResetPasswordErr::NotFound
    );
    user.set_password(new_password);

    repo_user::update(&user, conn).await?;

    biz_ok!(())
}
