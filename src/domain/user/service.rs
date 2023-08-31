use utils::db_pools::postgres::PgConn;

use crate::{
    biz_ok,
    domain::user::Email,
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{email::EmailCodeSender, repo_user},
    pg_tx,
};
use derive_more::From;

use super::{
    EmailFormatErr, Password, PasswordFormatErr, PasswordNotMatch, Phone, PhoneFormatErr, User,
    UserId, UserName, UserNameFormatErr,
};

#[derive(derive_more::From)]
pub enum RegisterErr {
    Name(UserNameFormatErr),
    Password(PasswordFormatErr),
    Email(EmailFormatErr),
    AlreadyRegister,
    EmailCodeMisMatch,
}

pub async fn register(user: User) -> BizResult<UserId, RegisterErr> {
    pg_tx!(register_tx, user)
}

pub async fn register_tx(user: User, conn: &mut PgConn) -> BizResult<UserId, RegisterErr> {
    ensure_biz!(not repo_user::exist(&user.email, conn).await?, RegisterErr::AlreadyRegister);
    // 上一步检查有概率漏检，所以应该以最后一步写入结果为准
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
    user.reset_password(new_password);

    repo_user::update(&user, conn).await?;

    biz_ok!(())
}

pub struct UserUpdate {
    pub user_name: Option<UserName>,
    pub password: Option<UpdatePassword>,
    pub address: Option<String>,
    pub mobile_number: Option<Phone>,
}

pub struct UpdatePassword {
    pub old_password: String,
    pub new_password: Password,
}

#[derive(From)]
pub enum UpdateProfileErr {
    Name(UserNameFormatErr),
    Password(PasswordFormatErr),
    Phone(PhoneFormatErr),
    NotFound,
    SmsCodeMismatch,
    PasswordWrong(PasswordNotMatch),
    PhoneAlreadyBinded,
}

pub async fn update_profile(
    user_id: UserId,
    update_info: UserUpdate,
    conn: &mut PgConn,
) -> BizResult<(), UpdateProfileErr> {
    if let Some(phone) = &update_info.mobile_number {
        // 目前单个手机号只能绑定一个账号
        ensure_biz!(
            not repo_user::exist(phone, conn).await?,
            UpdateProfileErr::PhoneAlreadyBinded
        );
    }

    let mut user = ensure_exist!(
        repo_user::find(user_id, conn).await?,
        UpdateProfileErr::NotFound
    );

    ensure_biz!(user.update_profile(update_info).await?);

    repo_user::update(&user, conn).await?;
    biz_ok!(())
}
