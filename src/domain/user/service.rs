use anyhow::bail;
use utils::db_pools::postgres::PgConn;

use crate::{
    biz_ok,
    domain::user::Email,
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{email, repo_user},
    pg_tx,
};

use super::{EmailFormatErr, PasswordFormatErr, User, UserId, UserNameFormatErr};

#[derive(derive_more::From)]
pub enum RegisterErr {
    Name(UserNameFormatErr),
    Password(PasswordFormatErr),
    Email(EmailFormatErr),
    AlreadyRegister,
    NoEmailCode,
    EmailCodeMisMatch,
}

pub async fn register(user: User, email_code: String) -> BizResult<UserId, RegisterErr> {
    let code = ensure_exist!(
        email::EmailCodeSender::get_sent_code(&user.email).await?,
        RegisterErr::NoEmailCode
    );
    let code = code.to_string();
    ensure_biz!(code == email_code, RegisterErr::EmailCodeMisMatch);

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
    repo_user::save_changed(&user, conn).await?;

    biz_ok!(user.id)
}

pub async fn logout_tx(user_id: UserId, conn: &mut PgConn) -> anyhow::Result<()> {
    let Some(mut user) = repo_user::find(user_id, conn).await? else {
        bail!("user not found. id = {}", user_id);
    };

    user.logout();
    repo_user::save_changed(&user, conn).await?;

    Ok(())
}
