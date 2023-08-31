use serde::Deserialize;
use utils::db_pools::postgres::{pg_conn, PgConn};

use crate::{
    biz_ok,
    domain::user::{
        service::{
            self, login_tx, LoginErr, RegisterErr, ResetPasswordErr, SanityCheck, UpdateProfileErr,
        },
        service_email::{self, CheckEmailCodeErr, SendEmailCodeErr},
        Email, Password, Phone, PhoneFormatErr, User, UserId, UserName,
    },
    ensure_biz, ensure_exist,
    http::BizResult,
    infrastructure::{email::EmailCodeSender, repo_user, sms_code::SmsSender},
    pg_tx, tx_func,
};
use anyhow::{bail, Result};
use derive_more::From;

pub async fn is_email_registerd(email: String) -> Result<bool> {
    let Ok(email) = Email::try_from(email) else {
        return Ok(false);
    };
    let conn = &mut pg_conn().await?;
    let exist = repo_user::exist(&email, conn).await?;
    Ok(exist)
}

pub async fn check_email_code(email: String, code: String) -> BizResult<bool, CheckEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    service_email::check_email_code(email, &code).await
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
    ensure_biz!(
        EmailCodeSender::verify_email_code(&email, &user_dto.email_code).await?,
        SanityCheck::EmailCodeNotMatch
    );

    let user = User::create(email, password);
    service::register(user).await
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
    tx_func!(logout_tx, id)
}

pub async fn logout_tx(user_id: UserId, conn: &mut PgConn) -> anyhow::Result<()> {
    let Some(mut user) = repo_user::find(user_id, conn).await? else {
        bail!("user not found. id = {}", user_id);
    };

    user.logout();

    repo_user::update(&user, conn).await?;

    Ok(())
}

pub async fn send_email_code(email: String, fake: bool) -> BizResult<(), SendEmailCodeErr> {
    let email = ensure_biz!(Email::try_from(email));
    service_email::send_email_code(email, fake).await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordDto {
    email: String,
    new_password: String,
    email_code: String,
}

pub async fn reset_password(params: ResetPasswordDto) -> BizResult<(), ResetPasswordErr> {
    let email = ensure_biz!(Email::try_from(params.email));
    let new_password = ensure_biz!(Password::try_from_async(params.new_password).await);
    service::reset_password(email, new_password, params.email_code).await
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateDto {
    pub user_name: Option<String>,
    pub password: Option<UpdatePassword>,
    pub address: Option<Vec<String>>,
    pub mobile_number: Option<MobileNumber>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePassword {
    old_password: String,
    new_password: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MobileNumber {
    // 管理员可以不传这个参数
    #[serde(default)]
    pub sms_code: String,
    pub tel: String,
}

pub async fn update_profile(
    user_id: UserId,
    update_info: UserUpdateDto,
) -> BizResult<(), UpdateProfileErr> {
    let phone = if let Some(phone_params) = update_info.mobile_number {
        let phone = ensure_biz!(Phone::try_from(phone_params.tel));
        ensure_biz!(
            SmsSender::verify(&phone, phone_params.sms_code).await?,
            SanityCheck::SmsCodeNotMatch
        );
        Some(phone)
    } else {
        None
    };
    let user_name = if let Some(name) = update_info.user_name {
        Some(ensure_biz!(UserName::try_from(name)))
    } else {
        None
    };

    let password = if let Some(password) = update_info.password {
        let p = service::UpdatePassword {
            old_password: password.old_password,
            new_password: ensure_biz!(Password::try_from_async(password.new_password).await),
        };
        Some(p)
    } else {
        None
    };

    let update_info = service::UserUpdate {
        user_name,
        password,
        address: update_info.address.map(|a| a.join(",")),
        mobile_number: phone,
    };

    pg_tx!(service::update_profile, user_id, update_info)
}

#[derive(From)]
pub enum SendSmsCodeErr {
    Phone(PhoneFormatErr),
    TooFrequent,
}

pub async fn send_sms_code(tel: String, fake: bool) -> BizResult<(), SendSmsCodeErr> {
    let tel = ensure_biz!(Phone::try_from(tel));
    let sender = ensure_exist!(
        SmsSender::try_build(&tel, fake).await?,
        SendSmsCodeErr::TooFrequent
    );
    sender.send().await?;

    biz_ok!(())
}
